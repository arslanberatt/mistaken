// Spec 05 whisper.cpp adapter process (docs/specs/spec-05-asr-benchmark-license-gate.md
// section 6, "Produced — adapter process protocol"). whisper.cpp has no
// native streaming recognizer (its own streaming example re-decodes a
// rolling window), so this adapter reports `streamingMode:
// "simulated-rolling-window"` in its `ready` event and must never present
// its rolling-window partials as native streaming latency (section 3,
// "Verified Current Behavior").
//
// Design: feed audio in chunkMs blocks (sleeping between blocks only in
// `realtime` pace, matching the protocol's pacing contract); every ~1 s of
// newly fed audio, re-decode the last `windowMs` of the accumulated buffer
// from scratch and emit the result as a `partial`. After all audio is fed,
// run one full-utterance decode over the entire clip and emit it as the
// single authoritative `final` — the rolling-window partials exist only to
// produce an honestly-labeled simulated first-partial-latency number, not
// to drive the scored transcript.

#include <whisper.h>

#include <algorithm>
#include <chrono>
#include <cstdio>
#include <filesystem>
#include <iostream>
#include <string>
#include <sys/resource.h>
#include <thread>
#include <vector>

#include "json_lite.h"
#include "wav_reader.h"

using Clock = std::chrono::steady_clock;

namespace {

long long ElapsedMs(Clock::time_point t0) {
  return std::chrono::duration_cast<std::chrono::milliseconds>(Clock::now() -
                                                                 t0)
      .count();
}

void EmitReady(long long at_ms) {
  std::cout << "{\"type\":\"ready\",\"atMs\":" << at_ms
            << ",\"streamingMode\":\"simulated-rolling-window\"}\n";
  std::cout.flush();
}

void EmitPartial(long long at_ms, const std::string &text) {
  std::cout << "{\"type\":\"partial\",\"atMs\":" << at_ms << ",\"text\":\""
            << json_lite::Escape(text) << "\"}\n";
  std::cout.flush();
}

void EmitFinal(long long at_ms, const std::string &text, int segment_index) {
  std::cout << "{\"type\":\"final\",\"atMs\":" << at_ms << ",\"text\":\""
            << json_lite::Escape(text) << "\",\"segmentIndex\":"
            << segment_index << "}\n";
  std::cout.flush();
}

void EmitMetrics(long long at_ms, long long audio_ms, long long wall_ms,
                  long long cpu_time_ms, long long peak_rss_bytes,
                  int fed_chunks, int dropped_chunks) {
  std::cout << "{\"type\":\"metrics\",\"atMs\":" << at_ms << ",\"audioMs\":"
            << audio_ms << ",\"wallMs\":" << wall_ms << ",\"cpuTimeMs\":"
            << cpu_time_ms << ",\"peakRssBytes\":" << peak_rss_bytes
            << ",\"fedChunks\":" << fed_chunks << ",\"droppedChunks\":"
            << dropped_chunks << "}\n";
  std::cout.flush();
}

void EmitError(long long at_ms, const std::string &code,
                const std::string &message) {
  std::cout << "{\"type\":\"error\",\"atMs\":" << at_ms << ",\"code\":\""
            << json_lite::Escape(code) << "\",\"message\":\""
            << json_lite::Escape(message) << "\"}\n";
  std::cout.flush();
}

long long PeakRssBytes() {
  struct rusage ru{};
  getrusage(RUSAGE_SELF, &ru);
#if defined(__APPLE__)
  return static_cast<long long>(ru.ru_maxrss);
#else
  return static_cast<long long>(ru.ru_maxrss) * 1024;
#endif
}

long long CpuTimeMs() {
  struct rusage ru{};
  getrusage(RUSAGE_SELF, &ru);
  auto to_ms = [](const struct timeval &tv) {
    return static_cast<long long>(tv.tv_sec) * 1000 + tv.tv_usec / 1000;
  };
  return to_ms(ru.ru_utime) + to_ms(ru.ru_stime);
}

std::string DecodeFull(struct whisper_context *ctx,
                        const std::vector<float> &samples, int num_threads) {
  whisper_full_params params =
      whisper_full_default_params(WHISPER_SAMPLING_GREEDY);
  params.print_progress = false;
  params.print_realtime = false;
  params.print_special = false;
  params.print_timestamps = false;
  params.translate = false;
  params.no_context = true;
  params.single_segment = false;
  params.n_threads = num_threads;
  params.language = "en";
  params.suppress_blank = true;

  if (whisper_full(ctx, params, samples.data(),
                    static_cast<int>(samples.size())) != 0) {
    return "";
  }
  std::string text;
  const int n = whisper_full_n_segments(ctx);
  for (int i = 0; i < n; ++i) {
    if (i > 0) text += " ";
    text += whisper_full_get_segment_text(ctx, i);
  }
  // Trim leading/trailing whitespace whisper.cpp segments commonly include.
  size_t start = text.find_first_not_of(' ');
  size_t end = text.find_last_not_of(' ');
  if (start == std::string::npos) return "";
  return text.substr(start, end - start + 1);
}

}  // namespace

int main() {
  // Protocol rule: stdout carries NDJSON events only, stderr is
  // diagnostic-only. ggml/whisper.cpp log to stdout by default; redirect
  // every message to stderr instead of letting it corrupt the event stream.
  auto log_to_stderr = [](ggml_log_level, const char *text, void *) {
    std::fputs(text, stderr);
  };
  ggml_log_set(log_to_stderr, nullptr);
  whisper_log_set(log_to_stderr, nullptr);

  const Clock::time_point t_process_start = Clock::now();

  std::string job_line;
  if (!std::getline(std::cin, job_line) || job_line.empty()) {
    EmitError(0, "protocol_error", "no job received on stdin");
    return 1;
  }


  const auto wav_path = json_lite::ExtractString(job_line, "wavPath");
  const auto model_dir = json_lite::ExtractString(job_line, "modelDir");
  const auto pace = json_lite::ExtractString(job_line, "pace").value_or("asap");
  const long long num_threads =
      json_lite::ExtractInt(job_line, "numThreads").value_or(2);
  const long long chunk_ms = json_lite::ExtractInt(job_line, "chunkMs").value_or(100);
  const long long window_ms = json_lite::ExtractInt(job_line, "windowMs").value_or(5000);

  if (!wav_path || !model_dir) {
    EmitError(0, "protocol_error", "job missing wavPath or model.modelDir");
    return 1;
  }
  if (!std::filesystem::exists(*model_dir)) {
    EmitError(0, "model_missing", "model directory not found: " + *model_dir);
    return 1;
  }
  std::string model_path;
  for (const auto &entry : std::filesystem::directory_iterator(*model_dir)) {
    const std::string name = entry.path().filename().string();
    if (name.size() > 4 && name.compare(name.size() - 4, 4, ".bin") == 0) {
      model_path = entry.path().string();
      break;
    }
  }
  if (model_path.empty()) {
    EmitError(0, "model_missing", "no .bin model file found under " + *model_dir);
    return 1;
  }

  auto wav = wav_reader::Read(*wav_path);
  if (!wav) {
    EmitError(0, "corpus_invalid", "failed to read wav file: " + *wav_path);
    return 1;
  }
  std::vector<float> samples = wav->samples;
  if (wav->sample_rate != WHISPER_SAMPLE_RATE) {
    samples = wav_reader::ResampleLinear(samples, wav->sample_rate,
                                         WHISPER_SAMPLE_RATE);
  }
  const int32_t sample_rate = WHISPER_SAMPLE_RATE;
  const long long audio_ms =
      static_cast<long long>(samples.size()) * 1000 / sample_rate;

  whisper_context_params cparams = whisper_context_default_params();
  cparams.use_gpu = false;
  struct whisper_context *ctx =
      whisper_init_from_file_with_params(model_path.c_str(), cparams);
  if (!ctx) {
    EmitError(0, "model_load_failed",
              "whisper_init_from_file_with_params returned null for " +
                  model_path);
    return 1;
  }

  EmitReady(0);
  const Clock::time_point t0 = Clock::now();

  const int32_t chunk_samples =
      std::max<int32_t>(1, static_cast<int32_t>(chunk_ms) * sample_rate / 1000);
  const int32_t window_samples =
      std::max<int32_t>(1, static_cast<int32_t>(window_ms) * sample_rate / 1000);
  const long long partial_decode_interval_ms = 1000;

  int fed_chunks = 0;
  std::string last_partial;
  long long last_partial_decode_at_ms = -partial_decode_interval_ms;

  size_t offset = 0;
  while (offset < samples.size()) {
    const size_t n = std::min<size_t>(chunk_samples, samples.size() - offset);
    offset += n;
    ++fed_chunks;

    const long long at_ms = ElapsedMs(t0);
    if (at_ms - last_partial_decode_at_ms >= partial_decode_interval_ms) {
      const size_t window_start =
          offset > static_cast<size_t>(window_samples)
              ? offset - static_cast<size_t>(window_samples)
              : 0;
      std::vector<float> window(samples.begin() + static_cast<long>(window_start),
                                samples.begin() + static_cast<long>(offset));
      const std::string text =
          DecodeFull(ctx, window, static_cast<int>(num_threads));
      if (!text.empty() && text != last_partial) {
        EmitPartial(ElapsedMs(t0), text);
        last_partial = text;
      }
      last_partial_decode_at_ms = ElapsedMs(t0);
    }

    if (pace == "realtime") {
      const long long budget_ms = chunk_ms;
      const long long spent_ms = ElapsedMs(t0) - at_ms;
      if (spent_ms < budget_ms) {
        std::this_thread::sleep_for(
            std::chrono::milliseconds(budget_ms - spent_ms));
      }
    }
  }

  // One authoritative full-utterance decode over the entire clip; this,
  // not the rolling-window partials, is the scored hypothesis.
  const std::string final_text =
      DecodeFull(ctx, samples, static_cast<int>(num_threads));
  if (!final_text.empty()) {
    EmitFinal(ElapsedMs(t0), final_text, 0);
  }

  whisper_free(ctx);

  const long long wall_ms = ElapsedMs(t0);
  EmitMetrics(ElapsedMs(t_process_start), audio_ms, wall_ms, CpuTimeMs(),
              PeakRssBytes(), fed_chunks, /*dropped_chunks=*/0);

  return 0;
}
