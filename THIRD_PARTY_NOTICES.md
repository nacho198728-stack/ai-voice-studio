# Third-party dependency notices

AI Voice Studio itself remains unlicensed as stated in `LICENSE.md`. The Phase
0.5 logging implementation incorporates the following separately licensed
open-source dependencies. Exact Rust package checksums are recorded in
`Cargo.lock`; native source integrity is recorded in `CMakeLists.txt`.

## Tokio tracing family

- `tracing` 0.1.44
- `tracing-subscriber` 0.3.23
- `tracing-appender` 0.2.5
- Source: <https://github.com/tokio-rs/tracing>
- License: MIT

Copyright (c) 2019 Tokio Contributors. Permission is granted under the MIT
License; the complete license text is distributed in each pinned Cargo package
and at <https://github.com/tokio-rs/tracing/blob/tracing-0.1.44/LICENSE>.

## spdlog and bundled fmt

- spdlog release: 1.16.0
- Official release commit: `486b55554f11c9cccc913e11a87085b2a91f706f`
- Download SHA-256: `d2fef585c9879dd239dc498e2e8a1e22982b3ed67b2d14e78622b7ef25bdfdfa`
- Source: <https://github.com/gabime/spdlog>
- License: MIT

Copyright (c) 2016 Gabi Melman. spdlog includes bundled fmt 12.0.0 headers;
fmt is Copyright (c) 2012-present Victor Zverovich and fmt contributors and is
also MIT-licensed. The spdlog archive's `LICENSE` records both obligations; the
fmt license is published at <https://github.com/fmtlib/fmt/blob/12.0.0/LICENSE>.

## MIT License text

Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the "Software"), to deal in
the Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software is furnished to do so,
subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
