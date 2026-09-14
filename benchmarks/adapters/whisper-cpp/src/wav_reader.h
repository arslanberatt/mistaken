// Minimal mono 16-bit PCM WAV reader for this adapter's corpus clips
// (`benchmarks/corpus/manifest.schema.json` fixes `codec: pcm_s16le,
// channels: 1`). whisper.cpp's public API has no bundled WAV loader (unlike
// sherpa-onnx's `SherpaOnnxReadWave`), so this adapter carries its own.
#pragma once

#include <cstdint>
#include <cstring>
#include <fstream>
#include <optional>
#include <string>
#include <vector>

namespace wav_reader {

struct Wav {
  int32_t sample_rate = 0;
  std::vector<float> samples;  // mono, normalized to [-1, 1]
};

inline std::optional<Wav> Read(const std::string &path) {
  std::ifstream f(path, std::ios::binary);
  if (!f) return std::nullopt;

  char riff[4];
  f.read(riff, 4);
  if (f.gcount() != 4 || std::memcmp(riff, "RIFF", 4) != 0) return std::nullopt;
  f.seekg(4, std::ios::cur);  // chunk size, unused
  char wave[4];
  f.read(wave, 4);
  if (f.gcount() != 4 || std::memcmp(wave, "WAVE", 4) != 0) return std::nullopt;

  Wav out;
  int16_t num_channels = 1;
  int16_t bits_per_sample = 16;
  bool have_fmt = false;

  while (f) {
    char chunk_id[4];
    f.read(chunk_id, 4);
    if (f.gcount() != 4) break;
    uint32_t chunk_size = 0;
    f.read(reinterpret_cast<char *>(&chunk_size), 4);
    if (f.gcount() != 4) break;

    if (std::memcmp(chunk_id, "fmt ", 4) == 0) {
      int16_t audio_format = 0;
      f.read(reinterpret_cast<char *>(&audio_format), 2);
      f.read(reinterpret_cast<char *>(&num_channels), 2);
      int32_t sample_rate = 0;
      f.read(reinterpret_cast<char *>(&sample_rate), 4);
      out.sample_rate = sample_rate;
      f.seekg(6, std::ios::cur);  // byte rate (4) + block align (2)
      f.read(reinterpret_cast<char *>(&bits_per_sample), 2);
      have_fmt = true;
      const uint32_t consumed = 16;
      if (chunk_size > consumed) f.seekg(chunk_size - consumed, std::ios::cur);
    } else if (std::memcmp(chunk_id, "data", 4) == 0) {
      if (!have_fmt || bits_per_sample != 16) return std::nullopt;
      const size_t num_frames = chunk_size / (num_channels * 2);
      std::vector<int16_t> raw(num_frames * num_channels);
      f.read(reinterpret_cast<char *>(raw.data()),
             static_cast<std::streamsize>(raw.size() * sizeof(int16_t)));
      out.samples.resize(num_frames);
      for (size_t i = 0; i < num_frames; ++i) {
        // Downmix to mono by averaging channels (corpus clips are already
        // mono per the manifest schema; this is a defensive fallback).
        int32_t acc = 0;
        for (int c = 0; c < num_channels; ++c) {
          acc += raw[i * num_channels + c];
        }
        out.samples[i] = static_cast<float>(acc / num_channels) / 32768.0f;
      }
      if (chunk_size % 2 == 1) f.seekg(1, std::ios::cur);  // pad byte
    } else {
      f.seekg(chunk_size, std::ios::cur);
    }
  }

  if (!have_fmt || out.samples.empty() || out.sample_rate <= 0) {
    return std::nullopt;
  }
  return out;
}

// Simple linear resampler, used only for the corpus's 48 kHz duplicate
// subset (whisper.cpp's public API always expects 16 kHz input; unlike
// sherpa-onnx it performs no internal resampling).
inline std::vector<float> ResampleLinear(const std::vector<float> &in,
                                          int32_t in_rate, int32_t out_rate) {
  if (in_rate == out_rate || in.empty()) return in;
  const double ratio = static_cast<double>(out_rate) / in_rate;
  const size_t out_len = static_cast<size_t>(in.size() * ratio);
  std::vector<float> out(out_len);
  for (size_t i = 0; i < out_len; ++i) {
    const double src_pos = i / ratio;
    const size_t i0 = static_cast<size_t>(src_pos);
    const size_t i1 = std::min(i0 + 1, in.size() - 1);
    const double frac = src_pos - i0;
    out[i] = static_cast<float>(in[i0] * (1.0 - frac) + in[i1] * frac);
  }
  return out;
}

}  // namespace wav_reader
