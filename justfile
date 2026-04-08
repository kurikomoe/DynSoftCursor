test:
  cargo test -- --no-capture

build: 
  pnpm tauri build --no-bundle


dev:
  pnpm tauri dev


release:
  pnpm tauri build
