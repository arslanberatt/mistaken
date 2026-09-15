// Spec 05 remediation candidate-expansion adapter (spec/05-remediation,
// docs/context/progress-tracker.md, "candidate set expanded, no gate
// relaxed"). Vosk (github.com/alphacep/vosk-api, Apache-2.0) is a
// genuinely different architecture from every previously frozen candidate:
// a Kaldi-derived HMM-DNN hybrid acoustic model decoded through a
// WFST (HCLr.fst/Gr.fst) graph with an n-gram language model baked into
// the graph, not an attention-based transducer (sherpa-onnx) or an
// autoregressive encoder-decoder (whisper.cpp). Native streaming: Vosk's
// C API (`vosk_recognizer_accept_waveform_s`) is designed from the ground
// up for real-time partial/final results, unlike whisper.cpp's
// full-utterance decode.
//
// Standalone executable: reads one JSON job from stdin, streams the
// clip's WAV through a real Vosk recognizer, and writes newline-delimited
// JSON events to stdout. No network access. No text transformation beyond
// what the recognizer itself produces: Vosk's own JSON result/partial
// fields are copied verbatim into this adapter's `final`/`partial`
// events — no lowercasing, punctuation, spell-correction, or word
// substitution here (normalization happens once, later, in the harness
// scorer, exactly like the other two adapters).

#include <vosk_api.h>

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

namespace fs = std::filesystem;
using Clock = std::chrono::steady_clock;

namespace {

long long ElapsedMs(Clock::time_point t0) {
  return std::chrono::duration_cast<std::chrono::milliseconds>(Clock::now() -
                                                                 t0)
      .count();
}

void EmitReady(long long at_ms) {
  std::cout << "{\"type\":\"ready\",\"atMs\":" << at_ms
            << ",\"streamingMode\":\"native-streaming\"}\n"
            << std::flush;
}

void EmitPartial(long long at_ms, const std::string &text) {
  std::cout << "{\"type\":\"partial\",\"atMs\":" << at_ms << ",\"text\":\""
            << json_lite::Escape(text) << "\"}\n"
            << std::flush;
}

void EmitFinal(long long at_ms, const std::string &text, int segment_index) {
  std::cout << "{\"type\":\"final\",\"atMs\":" << at_ms << ",\"text\":\""
            << json_lite::Escape(text) << "\",\"segmentIndex\":"
            << segment_index << "}\n"
            << std::flush;
}

void EmitMetrics(long long at_ms, long long audio_ms, long long wall_ms,
                  long long cpu_time_ms, long long peak_rss_bytes,
                  int fed_chunks, int dropped_chunks) {
  std::cout << "{\"type\":\"metrics\",\"atMs\":" << at_ms
            << ",\"audioMs\":" << audio_ms << ",\"wallMs\":" << wall_ms
            << ",\"cpuTimeMs\":" << cpu_time_ms
            << ",\"peakRssBytes\":" << peak_rss_bytes
            << ",\"fedChunks\":" << fed_chunks
            << ",\"droppedChunks\":" << dropped_chunks << "}\n"
            << std::flush;
}

void EmitError(long long at_ms, const std::string &code,
                const std::string &message) {
  std::cout << "{\"type\":\"error\",\"atMs\":" << at_ms << ",\"code\":\""
            << code << "\",\"message\":\"" << json_lite::Escape(message)
            << "\"}\n"
            << std::flush;
}

// `ru_maxrss` is bytes on macOS/BSD but kilobytes on Linux. Both of Spec
// 05's reference hosts (mac-arm64, win-x64) never hit the Linux branch,
// but the distinction is documented here because it is a classic footgun
// (same note as the sherpa-onnx and whisper-cpp adapters).
long long PeakRssBytes() {
  struct rusage ru;
  if (getrusage(RUSAGE_SELF, &ru) != 0) return 0;
#if defined(__APPLE__)
  return static_cast<long long>(ru.ru_maxrss);
#else
  return static_cast<long long>(ru.ru_maxrss) * 1024;
#endif
}

long long CpuTimeMs() {
  struct rusage ru;
  if (getrusage(RUSAGE_SELF, &ru) != 0) return 0;
  const long long user_ms =
      static_cast<long long>(ru.ru_utime.tv_sec) * 1000 +
      ru.ru_utime.tv_usec / 1000;
  const long long sys_ms = static_cast<long long>(ru.ru_stime.tv_sec) * 1000 +
                            ru.ru_stime.tv_usec / 1000;
  return user_ms + sys_ms;
}

}  // namespace

int main() {
  // Kaldi/Vosk log to stdout by default; keep the NDJSON event stream
  // clean by silencing informational logging (protocol rule: stdout
  // carries NDJSON events only, stderr is diagnostic-only). Real errors
  // still surface through this adapter's own `error` events.
  vosk_set_log_level(-1);

  const Clock::time_point t_process_start = Clock::now();

  std::string job_line;
  if (!std::getline(std::cin, job_line) || job_line.empty()) {
    EmitError(0, "protocol_error", "no job received on stdin");
    return 1;
  }

  const auto wav_path = json_lite::ExtractString(job_line, "wavPath");
  const auto model_dir = json_lite::ExtractString(job_line, "modelDir");
  const auto pace = json_lite::ExtractString(job_line, "pace").value_or("asap");
  const long long chunk_ms = json_lite::ExtractInt(job_line, "chunkMs").value_or(100);

  if (!wav_path || !model_dir) {
    EmitError(0, "protocol_error", "job missing wavPath or model.modelDir");
    return 1;
  }
  if (!fs::exists(*model_dir)) {
    EmitError(0, "model_missing", "model directory not found: " + *model_dir);
    return 1;
  }

  const auto wav = wav_reader::Read(*wav_path);
  if (!wav) {
    EmitError(0, "corpus_invalid", "failed to read wav file: " + *wav_path);
    return 1;
  }
  const int32_t sample_rate = wav->sample_rate;
  const long long audio_ms =
      sample_rate > 0
          ? static_cast<long long>(wav->samples.size()) * 1000 / sample_rate
          : 0;

  VoskModel *model = vosk_model_new(model_dir->c_str());
  if (!model) {
    EmitError(0, "model_load_failed",
              "vosk_model_new returned null for " + *model_dir);
    return 1;
  }

  // Vosk resamples internally to the model's trained rate from whatever
  // `sample_rate` is declared here — the adapter never resamples the
  // corpus's 48 kHz duplicate subset itself (same convention as the
  // sherpa-onnx adapter's "resample once, inside the recognizer" rule).
  VoskRecognizer *recognizer =
      vosk_recognizer_new(model, static_cast<float>(sample_rate));
  if (!recognizer) {
    EmitError(0, "model_load_failed", "vosk_recognizer_new returned null");
    vosk_model_free(model);
    return 1;
  }
  vosk_recognizer_set_words(recognizer, 0);

  EmitReady(0);
  const Clock::time_point t0 = Clock::now();

  const int32_t chunk_samples =
      std::max<int32_t>(1, static_cast<int32_t>(chunk_ms) * sample_rate / 1000);
  int fed_chunks = 0;
  int segment_index = 0;
  std::string last_partial;

  size_t offset = 0;
  while (offset < wav->samples.size()) {
    const size_t n =
        std::min<size_t>(chunk_samples, wav->samples.size() - offset);
    const int accept_result = vosk_recognizer_accept_waveform_s(
        recognizer, wav->samples.data() + offset, static_cast<int>(n));
    offset += n;
    ++fed_chunks;

    const long long at_ms = ElapsedMs(t0);
    if (accept_result == 1) {
      // Endpoint reached: `result()` returns the finalized utterance.
      const std::string result_json = vosk_recognizer_result(recognizer);
      const std::string text =
          json_lite::ExtractString(result_json, "text").value_or("");
      if (!text.empty()) {
        EmitFinal(at_ms, text, segment_index++);
      }
      last_partial.clear();
    } else if (accept_result == 0) {
      const std::string partial_json = vosk_recognizer_partial_result(recognizer);
      const std::string text =
          json_lite::ExtractString(partial_json, "partial").value_or("");
      if (!text.empty() && text != last_partial) {
        EmitPartial(at_ms, text);
        last_partial = text;
      }
    } else {
      EmitError(at_ms, "protocol_error",
                "vosk_recognizer_accept_waveform_s returned -1");
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

  // Flush trailing decoded text: a clip's final segment often ends at EOF
  // without enough trailing silence to trip Vosk's endpointer, and that
  // text must still reach the scorer (same rule as the sherpa-onnx
  // adapter's post-loop `InputFinished` flush).
  {
    const std::string final_json = vosk_recognizer_final_result(recognizer);
    const std::string text =
        json_lite::ExtractString(final_json, "text").value_or("");
    if (!text.empty()) {
      EmitFinal(ElapsedMs(t0), text, segment_index++);
    }
  }

  // Release recognizer and model handles before emitting metrics so the
  // reported peak RSS covers the real decode session (Spec 05 section 11,
  // "Process lifecycle").
  vosk_recognizer_free(recognizer);
  vosk_model_free(model);

  const long long wall_ms = ElapsedMs(t0);
  EmitMetrics(ElapsedMs(t_process_start), audio_ms, wall_ms, CpuTimeMs(),
              PeakRssBytes(), fed_chunks, /*dropped_chunks=*/0);

  return 0;
}
