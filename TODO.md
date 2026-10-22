# TODO

- [ ] Integrate libmpv's GPU render API with CreamUI textures on Linux and
  Windows so decoded video stays on the GPU instead of passing through software
  RGB buffers. Benchmark large videos, seeks, playback-rate changes, and A/V
  synchronization against the FFmpeg fallback before replacing libmpv's
  software rendering path by default.
