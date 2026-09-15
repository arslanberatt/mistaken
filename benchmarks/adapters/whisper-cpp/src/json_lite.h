// Minimal, non-general JSON field extraction/escaping for this adapter's
// fixed, self-controlled job/event schema (Spec 05 section 6, "Produced —
// adapter process protocol"). This is deliberately not a general JSON
// parser: the harness that produces job lines is this repository's own
// Rust code, so every key name below is known and unique within the job
// object, and a substring search on `"<key>"` is safe and sufficient.
#pragma once

#include <cctype>
#include <cstdio>
#include <optional>
#include <string>

namespace json_lite {

inline std::optional<std::string> ExtractString(const std::string &json,
                                                  const std::string &key) {
  const std::string needle = "\"" + key + "\"";
  auto pos = json.find(needle);
  if (pos == std::string::npos) return std::nullopt;
  pos = json.find(':', pos + needle.size());
  if (pos == std::string::npos) return std::nullopt;
  pos = json.find('"', pos);
  if (pos == std::string::npos) return std::nullopt;
  ++pos;
  std::string out;
  while (pos < json.size() && json[pos] != '"') {
    if (json[pos] == '\\' && pos + 1 < json.size()) {
      ++pos;
      switch (json[pos]) {
        case 'n': out.push_back('\n'); break;
        case 't': out.push_back('\t'); break;
        case '"': out.push_back('"'); break;
        case '\\': out.push_back('\\'); break;
        default: out.push_back(json[pos]); break;
      }
    } else {
      out.push_back(json[pos]);
    }
    ++pos;
  }
  return out;
}

inline std::optional<long long> ExtractInt(const std::string &json,
                                            const std::string &key) {
  const std::string needle = "\"" + key + "\"";
  auto pos = json.find(needle);
  if (pos == std::string::npos) return std::nullopt;
  pos = json.find(':', pos + needle.size());
  if (pos == std::string::npos) return std::nullopt;
  ++pos;
  while (pos < json.size() &&
         std::isspace(static_cast<unsigned char>(json[pos]))) {
    ++pos;
  }
  size_t start = pos;
  if (pos < json.size() && (json[pos] == '-' || json[pos] == '+')) ++pos;
  while (pos < json.size() &&
         std::isdigit(static_cast<unsigned char>(json[pos]))) {
    ++pos;
  }
  if (pos == start) return std::nullopt;
  return std::stoll(json.substr(start, pos - start));
}

inline std::optional<double> ExtractFloat(const std::string &json,
                                           const std::string &key) {
  const std::string needle = "\"" + key + "\"";
  auto pos = json.find(needle);
  if (pos == std::string::npos) return std::nullopt;
  pos = json.find(':', pos + needle.size());
  if (pos == std::string::npos) return std::nullopt;
  ++pos;
  while (pos < json.size() &&
         std::isspace(static_cast<unsigned char>(json[pos]))) {
    ++pos;
  }
  size_t start = pos;
  if (pos < json.size() && (json[pos] == '-' || json[pos] == '+')) ++pos;
  while (pos < json.size() &&
         (std::isdigit(static_cast<unsigned char>(json[pos])) ||
          json[pos] == '.' || json[pos] == 'e' || json[pos] == 'E' ||
          json[pos] == '-' || json[pos] == '+')) {
    ++pos;
  }
  if (pos == start) return std::nullopt;
  return std::stod(json.substr(start, pos - start));
}

inline std::optional<bool> ExtractBool(const std::string &json,
                                        const std::string &key) {
  const std::string needle = "\"" + key + "\"";
  auto pos = json.find(needle);
  if (pos == std::string::npos) return std::nullopt;
  pos = json.find(':', pos + needle.size());
  if (pos == std::string::npos) return std::nullopt;
  ++pos;
  while (pos < json.size() &&
         std::isspace(static_cast<unsigned char>(json[pos]))) {
    ++pos;
  }
  if (json.compare(pos, 4, "true") == 0) return true;
  if (json.compare(pos, 5, "false") == 0) return false;
  return std::nullopt;
}

// Escapes a string for embedding as a JSON string value (used only for our
// own emitted NDJSON events, where every field is either plain ASCII
// recognizer output or a controlled literal).
inline std::string Escape(const std::string &s) {
  std::string out;
  out.reserve(s.size());
  for (unsigned char c : s) {
    switch (c) {
      case '"': out += "\\\""; break;
      case '\\': out += "\\\\"; break;
      case '\n': out += "\\n"; break;
      case '\t': out += "\\t"; break;
      case '\r': out += "\\r"; break;
      default:
        if (c < 0x20) {
          char buf[8];
          std::snprintf(buf, sizeof(buf), "\\u%04x", c);
          out += buf;
        } else {
          out.push_back(static_cast<char>(c));
        }
    }
  }
  return out;
}

}  // namespace json_lite
