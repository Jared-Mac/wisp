# Private Quickshell web runtime

Upstream: https://github.com/quickshell-mirror/quickshell

Version: 0.3.1, commit `1a4716cde794a59928d9d9fc15f2afc7a95de360`.

The optional runtime is built from a checksum-verified upstream source archive
by `scripts/build-web-runtime.sh`. The full corresponding source archive and
patched source remain in the user's Wisp build cache. The two local changes in
`src/launch/launch.cpp` are:

```cpp
QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
auto qArgC = 1; // upstream uses 0
```

This supplies Chromium's required program argument and shared graphics context.
Wisp uses the separate `wisp-quickshell` executable; the system's Quickshell is
unchanged. Upstream licensing is LGPL-3.0, with GPL-3.0 terms incorporated by
reference; both license texts are included here. Users can rebuild, modify or
replace the private runtime without modifying the Wisp application.
