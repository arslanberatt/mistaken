// Spec 05 sherpa-onnx adapter process (docs/specs/spec-05-asr-benchmark-license-gate.md
// section 6, "Produced — adapter process protocol"). Standalone executable:
// reads one JSON job from stdin, streams the clip's WAV through a real
// sherpa-onnx OnlineRecognizer (native streaming, greedy_search), and
// writes newline-delimited JSON events to stdout. No network access. No
// text transformation beyond what the recognizer itself produces: no
// lowercasing, punctuation, spell-correction, or word substitution here —
// normalization happens once, later, in the harness scorer.

#include <sherpa-onnx/c-api/c-api.h>

#include <chrono>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <iostream>
#include <string>
#include <sys/resource.h>
#include <thread>
#include <vector>

#include "json_lite.h"

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
            << ",\"streamingMode\":\"native-streaming\"}\n";
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

// Locates the one file in `dir` whose name starts with `prefix` and ends
// with `.onnx`. The pinned candidate descriptors each ship exactly one
// encoder/decoder/joiner file per model directory (int8 XOR fp32), so a
// prefix match is unambiguous.
std::string FindModelFile(const std::string &dir, const std::string &prefix) {
  for (const auto &entry : fs::directory_iterator(dir)) {
    const std::string name = entry.path().filename().string();
    if (name.rfind(prefix, 0) == 0 && name.size() > 5 &&
        name.compare(name.size() - 5, 5, ".onnx") == 0) {
      return entry.path().string();
    }
  }
  return "";
}

// `ru_maxrss` is bytes on macOS/BSD but kilobytes on Linux. Both of Spec
// 05's reference hosts (mac-arm64, win-x64) never hit the Linux branch, but
// the distinction is documented here because it is a classic footgun.
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

}  // namespace

int main() {
  const Clock::time_point t_process_start = Clock::now();

  std::string job_line;
  if (!std::getline(std::cin, job_line) || job_line.empty()) {
    EmitError(0, "protocol_error", "no job received on stdin");
    return 1;
  }

  const auto clip_id = json_lite::ExtractString(job_line, "clipId").value_or("");
  const auto wav_path = json_lite::ExtractString(job_line, "wavPath");
  const auto model_dir = json_lite::ExtractString(job_line, "modelDir");
  const auto pace = json_lite::ExtractString(job_line, "pace").value_or("asap");
  const auto method =
      json_lite::ExtractString(job_line, "method").value_or("greedy_search");
  const long long num_threads =
      json_lite::ExtractInt(job_line, "numThreads").value_or(2);
  const long long chunk_ms = json_lite::ExtractInt(job_line, "chunkMs").value_or(100);

  if (!wav_path || !model_dir) {
    EmitError(0, "protocol_error", "job missing wavPath or model.modelDir");
    return 1;
  }
  if (!fs::exists(*model_dir)) {
    EmitError(0, "model_missing", "model directory not found: " + *model_dir);
    return 1;
  }

  const std::string encoder = FindModelFile(*model_dir, "encoder");
  const std::string decoder = FindModelFile(*model_dir, "decoder");
  const std::string joiner = FindModelFile(*model_dir, "joiner");
  const std::string tokens = (fs::path(*model_dir) / "tokens.txt").string();
  if (encoder.empty() || decoder.empty() || joiner.empty() ||
      !fs::exists(tokens)) {
    EmitError(0, "model_missing",
              "encoder/decoder/joiner/tokens.txt not all found under " +
                  *model_dir);
    return 1;
  }

  SherpaOnnxOnlineRecognizerConfig config;
  memset(&config, 0, sizeof(config));
  config.feat_config.sample_rate = 16000;
  config.feat_config.feature_dim = 80;
  config.model_config.transducer.encoder = encoder.c_str();
  config.model_config.transducer.decoder = decoder.c_str();
  config.model_config.transducer.joiner = joiner.c_str();
  config.model_config.tokens = tokens.c_str();
  config.model_config.provider = "cpu";
  config.model_config.num_threads = static_cast<int32_t>(num_threads);
  config.model_config.debug = 0;
  config.decoding_method = method.c_str();
  config.max_active_paths = 4;
  config.enable_endpoint = 1;
  // Mirrors Mistaken's frozen production endpoint rules
  // (docs/context/progress-tracker.md, "All rewriting features off by
  // construction"): 2.4 s / 1.0 s / 15 s.
  config.rule1_min_trailing_silence = 2.4f;
  config.rule2_min_trailing_silence = 1.0f;
  config.rule3_min_utterance_length = 15.0f;

  const SherpaOnnxOnlineRecognizer *recognizer =
      SherpaOnnxCreateOnlineRecognizer(&config);
  if (!recognizer) {
    EmitError(0, "model_load_failed", "SherpaOnnxCreateOnlineRecognizer returned null");
    return 1;
  }

  const SherpaOnnxWave *wave = SherpaOnnxReadWave(wav_path->c_str());
  if (!wave) {
    EmitError(0, "corpus_invalid", "failed to read wav file: " + *wav_path);
    SherpaOnnxDestroyOnlineRecognizer(recognizer);
    return 1;
  }
  const int32_t sample_rate = wave->sample_rate;
  const int32_t num_samples = wave->num_samples;
  const long long audio_ms =
      sample_rate > 0
          ? static_cast<long long>(num_samples) * 1000 / sample_rate
          : 0;

  const SherpaOnnxOnlineStream *stream = SherpaOnnxCreateOnlineStream(recognizer);

  // Optional cold-start mitigation (spec-05-remediation experiment,
  // benchmarks/reports/approval.md): feed `warmup_silence_ms` of
  // synthetic zero-valued audio through the recognizer before the clip's
  // real samples, so a streaming model's internal state has left context
  // to draw on before the first real word arrives. Done before
  // `EmitReady`/`t0` so it never appears in the protocol's measured
  // `atMs` timeline (the same treatment as model loading, which also
  // happens before `EmitReady`); it is not real audio and is never
  // real-time paced.
  const auto warmup_silence_ms =
      json_lite::ExtractInt(job_line, "warmupSilenceMs").value_or(0);
  if (warmup_silence_ms > 0) {
    const int32_t warmup_samples =
        static_cast<int32_t>(warmup_silence_ms) * sample_rate / 1000;
    const std::vector<float> silence(static_cast<size_t>(warmup_samples), 0.0f);
    SherpaOnnxOnlineStreamAcceptWaveform(stream, sample_rate, silence.data(),
                                         warmup_samples);
    while (SherpaOnnxIsOnlineStreamReady(recognizer, stream)) {
      SherpaOnnxDecodeOnlineStream(recognizer, stream);
    }
  }

  EmitReady(0);
  const Clock::time_point t0 = Clock::now();

  const int32_t chunk_samples =
      std::max<int32_t>(1, static_cast<int32_t>(chunk_ms) * sample_rate / 1000);
  int fed_chunks = 0;
  int segment_index = 0;
  std::string last_partial;
  bool any_final_emitted_this_segment = false;

  int32_t offset = 0;
  while (offset < num_samples) {
    const int32_t n = std::min(chunk_samples, num_samples - offset);
    SherpaOnnxOnlineStreamAcceptWaveform(stream, sample_rate,
                                         wave->samples + offset, n);
    offset += n;
    ++fed_chunks;

    while (SherpaOnnxIsOnlineStreamReady(recognizer, stream)) {
      SherpaOnnxDecodeOnlineStream(recognizer, stream);
    }

    const SherpaOnnxOnlineRecognizerResult *result =
        SherpaOnnxGetOnlineStreamResult(recognizer, stream);
    const std::string text = result->text ? result->text : "";
    SherpaOnnxDestroyOnlineRecognizerResult(result);

    const long long at_ms = ElapsedMs(t0);
    if (!text.empty() && text != last_partial) {
      EmitPartial(at_ms, text);
      last_partial = text;
      any_final_emitted_this_segment = false;
    }

    if (SherpaOnnxOnlineStreamIsEndpoint(recognizer, stream)) {
      if (!text.empty() && !any_final_emitted_this_segment) {
        EmitFinal(at_ms, text, segment_index++);
        any_final_emitted_this_segment = true;
      }
      SherpaOnnxOnlineStreamReset(recognizer, stream);
      last_partial.clear();
      any_final_emitted_this_segment = false;
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
  // without enough trailing silence to trip the endpoint rules, and that
  // text must still reach the scorer.
  SherpaOnnxOnlineStreamInputFinished(stream);
  while (SherpaOnnxIsOnlineStreamReady(recognizer, stream)) {
    SherpaOnnxDecodeOnlineStream(recognizer, stream);
  }
  {
    const SherpaOnnxOnlineRecognizerResult *result =
        SherpaOnnxGetOnlineStreamResult(recognizer, stream);
    const std::string text = result->text ? result->text : "";
    SherpaOnnxDestroyOnlineRecognizerResult(result);
    if (!text.empty()) {
      EmitFinal(ElapsedMs(t0), text, segment_index++);
    }
  }

  // Release recognizer, stream, and model handles before emitting metrics
  // so the reported peak RSS covers the real decode session (Spec 05
  // section 11, "Process lifecycle").
  SherpaOnnxDestroyOnlineStream(stream);
  SherpaOnnxDestroyOnlineRecognizer(recognizer);
  SherpaOnnxFreeWave(wave);

  const long long wall_ms = ElapsedMs(t0);
  EmitMetrics(ElapsedMs(t_process_start), audio_ms, wall_ms, CpuTimeMs(),
              PeakRssBytes(), fed_chunks, /*dropped_chunks=*/0);

  (void)clip_id;
  return 0;
}
