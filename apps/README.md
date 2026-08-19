# Apps

Owns desktop applications and host integrations. `desktop` is the Tauri product
adapter and frontend; `runtime-host` owns the Rust process supervisor. Neither
may take responsibility for realtime audio processing, engine loading, or
direct model execution.
