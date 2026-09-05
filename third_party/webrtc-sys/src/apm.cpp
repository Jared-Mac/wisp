/*
 * Copyright 2025 LiveKit, Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#include "livekit/apm.h"

#include "api/audio/builtin_audio_processing_builder.h"
#include "api/environment/environment_factory.h"

#include <algorithm>
#include <array>
#include <chrono>
#include <cmath>
#include <mutex>
#include <vector>
#include <iostream>
#include <memory>

namespace livekit_ffi {

// One reference subscriber per explicit APM. The RTC runtime is process-wide.
// Fixed storage keeps allocations and neural work off the render callback.
struct ReferenceFrame {
  std::array<int16_t, 480> samples{};
  int rate = 48000;
  std::chrono::steady_clock::time_point captured;
};

struct PlayoutReference {
  std::mutex mutex;
  std::array<ReferenceFrame, 10> frames;
  size_t begin = 0;
  size_t size = 0;
};

namespace {
std::mutex reference_mutex;
std::vector<std::weak_ptr<PlayoutReference>> reference_subscribers;

class PlayoutReferenceTap final : public webrtc::CustomProcessing {
 public:
  void Initialize(int sample_rate_hz, int num_channels) override {}
  std::string ToString() const override { return "Wisp speaker reference"; }
  void Process(webrtc::AudioBuffer* audio) override {
    if (!audio->num_channels() || !audio->num_frames() || audio->num_frames() > 480)
      return;
    // A short collision may drop a reference frame, never block speaker playout.
    std::unique_lock hub(reference_mutex, std::try_to_lock);
    if (!hub.owns_lock()) return;
    for (auto it = reference_subscribers.begin(); it != reference_subscribers.end();) {
      auto reference = it->lock();
      if (!reference) {
        it = reference_subscribers.erase(it);
        continue;
      }
      ++it;
      std::unique_lock lock(reference->mutex, std::try_to_lock);
      if (!lock.owns_lock()) continue;
      if (reference->size == reference->frames.size()) {
        reference->begin = (reference->begin + 1) % reference->frames.size();
        --reference->size;
      }
      auto& frame = reference->frames[(reference->begin + reference->size++) % reference->frames.size()];
      frame.rate = static_cast<int>(audio->num_frames() * 100);
      frame.captured = std::chrono::steady_clock::now();
      for (size_t i = 0; i < audio->num_frames(); ++i) {
        float sample = 0;
        for (size_t ch = 0; ch < audio->num_channels(); ++ch)
          sample += audio->channels_const()[ch][i];
        frame.samples[i] = static_cast<int16_t>(std::clamp(
            std::round(sample / audio->num_channels()), -32768.0f, 32767.0f));
      }
    }
  }
};
}  // namespace

std::unique_ptr<webrtc::CustomProcessing> create_playout_reference_tap() {
  return std::make_unique<PlayoutReferenceTap>();
}

void AudioProcessingModule::use_playout_reference(bool enabled) {
  playout_reference_.reset();
  playout_reference_frames_ = 0;
  if (enabled) {
    playout_reference_ = std::make_shared<PlayoutReference>();
    std::lock_guard lock(reference_mutex);
    reference_subscribers.erase(std::remove_if(reference_subscribers.begin(),
        reference_subscribers.end(), [](const auto& entry) { return entry.expired(); }),
        reference_subscribers.end());
    reference_subscribers.push_back(playout_reference_);
  }
}

uint64_t AudioProcessingModule::playout_reference_frames() const {
  return playout_reference_frames_;
}

void AudioProcessingModule::enable_high_noise_suppression() {
  auto config = apm_->GetConfig();
  if (config.noise_suppression.enabled &&
      config.noise_suppression.level == webrtc::AudioProcessing::Config::NoiseSuppression::kHigh)
    return;
  config.noise_suppression.enabled = true;
  config.noise_suppression.level = webrtc::AudioProcessing::Config::NoiseSuppression::kHigh;
  apm_->ApplyConfig(config);
}

AudioProcessingModule::AudioProcessingModule(
    const AudioProcessingConfig& config) {
  apm_ = webrtc::BuiltinAudioProcessingBuilder()
             .Build(webrtc::CreateEnvironment());

  apm_->ApplyConfig(config.ToWebrtcConfig());
  apm_->Initialize();
}

int AudioProcessingModule::process_stream(const int16_t* src,
                                          size_t src_len,
                                          int16_t* dst,
                                          size_t dst_len,
                                          int sample_rate,
                                          int num_channels) {
  if (playout_reference_) {
    std::array<ReferenceFrame, 10> frames;
    size_t count;
    {
      std::lock_guard lock(playout_reference_->mutex);
      count = playout_reference_->size;
      for (size_t i = 0; i < count; ++i)
        frames[i] = playout_reference_->frames[(playout_reference_->begin + i) % frames.size()];
      playout_reference_->begin = 0;
      playout_reference_->size = 0;
    }
    for (size_t i = 0; i < count; ++i) {
      auto& frame = frames[i];
      if (std::chrono::steady_clock::now() - frame.captured > std::chrono::milliseconds(100))
        continue;
      webrtc::StreamConfig render_cfg(frame.rate, 1);
      int result = apm_->ProcessReverseStream(frame.samples.data(), render_cfg,
                                             render_cfg, frame.samples.data());
      if (result != 0) return result;
      ++playout_reference_frames_;
    }
  }
  webrtc::StreamConfig stream_cfg(sample_rate, num_channels);
  return apm_->ProcessStream(src, stream_cfg, stream_cfg, dst);
}

int AudioProcessingModule::process_reverse_stream(const int16_t* src,
                                                  size_t src_len,
                                                  int16_t* dst,
                                                  size_t dst_len,
                                                  int sample_rate,
                                                  int num_channels) {
  webrtc::StreamConfig stream_cfg(sample_rate, num_channels);
  return apm_->ProcessReverseStream(src, stream_cfg, stream_cfg, dst);
}

int AudioProcessingModule::set_stream_delay_ms(int delay_ms) {
  return apm_->set_stream_delay_ms(delay_ms);
}

std::unique_ptr<AudioProcessingModule> create_apm(
    bool echo_canceller_enabled,
    bool gain_controller_enabled,
    bool high_pass_filter_enabled,
    bool noise_suppression_enabled) {
  AudioProcessingConfig config;
  config.echo_canceller_enabled = echo_canceller_enabled;
  config.gain_controller_enabled = gain_controller_enabled;
  config.high_pass_filter_enabled = high_pass_filter_enabled;
  config.noise_suppression_enabled = noise_suppression_enabled;
  return std::make_unique<AudioProcessingModule>(config);
}

}  // namespace livekit_ffi
