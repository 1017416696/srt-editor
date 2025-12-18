# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**VoSub** is a professional desktop subtitle editor built with Tauri 2, Vue 3, and Rust. It provides real-time subtitle editing, audio waveform visualization, and advanced text processing features for SRT (SubRip) subtitle files.

**Current Version**: 2.9.0 (Cargo.toml) | 2.6.0 (package.json)

## Technology Stack

### Frontend
- **Vue 3** with TypeScript and Composition API
- **Vite** (build tool with hot reload)
- **Pinia** (state management)
- **Tailwind CSS** + **Element Plus** (UI framework)
- **WaveSurfer.js** (audio visualization)
- **Howler.js** (audio playback)
- **Vue Router** (2 routes: `/` for welcome, `/editor` for main UI)

### Backend
- **Tauri 2** (cross-platform desktop framework)
- **Rust** with async/await via Tokio
- **Symphonia** (audio codec support: MP3, WAV, AAC, FLAC, OGG)
- **Serde** (JSON serialization)

### Development Tools
- **Vitest** (unit testing)
- **TypeScript** (strict mode enabled)
- **ESLint** + **Prettier** (code quality)
- **pnpm** (package manager)
- **GitHub Actions** (CI/CD)

## Directory Structure

```
src/                          # Frontend (Vue 3 + TypeScript)
├── App.vue                   # Root component
├── main.ts                   # Entry point
├── router/index.ts           # Routes config
├── stores/                   # Pinia stores
│   ├── subtitle.ts          # Core subtitle editing logic
│   ├── audio.ts             # Audio playback state
│   └── config.ts            # App configuration
├── components/               # Vue components
│   └── WaveformViewer.vue   # Audio visualization
├── views/
│   ├── WelcomePage.vue      # Landing page
│   └── EditorPage.vue       # Main editor UI
├── types/subtitle.ts        # TypeScript definitions
└── utils/time.ts            # Time formatting utilities

src-tauri/                     # Backend (Rust)
├── src/
│   ├── main.rs             # Entry point, menus, event listeners
│   ├── lib.rs              # Tauri command handlers (IPC)
│   ├── srt_parser.rs       # SRT file parsing/serialization
│   └── waveform_generator.rs # Audio analysis & waveform generation
├── Cargo.toml              # Rust dependencies
└── tauri.conf.json         # App configuration (window size, security, etc.)

tests/
├── unit/
│   └── example.test.ts      # Unit test examples
└── setup/
    ├── testglobals.ts       # Global test config
    └── install-pinia.ts     # Pinia test utilities
```

## Essential Commands

### Development

```bash
# Start development server (both frontend and backend)
pnpm tauri dev

# Run with Vue DevTools
pnpm dev:full

# Type checking
pnpm type-check

# Linting and formatting
pnpm lint              # Check with ESLint
pnpm format            # Format with Prettier
```

### Testing & Building

```bash
# Run unit tests
pnpm test

# Build production app
pnpm tauri build

# Type-check and build
pnpm build

# Single test file
pnpm vitest tests/unit/specific.test.ts
```

### Deployment

```bash
# Bump version (major.minor.patch)
pnpm bump 2.10.0

# Update Cargo.lock
pnpm check
```

## Architecture Overview

### Frontend State Management (Pinia)

The **subtitle store** (`src/stores/subtitle.ts`) is the core of the application:
- **State**: SRT entries, undo/redo history (max 100 actions), search results
- **Actions**: CRUD operations, time synchronization, batch processing, file I/O
- **Features**:
  - Real-time conflict detection (overlapping subtitles)
  - Search with result navigation
  - Batch operations (HTML tag removal, punctuation removal)
  - Full undo/redo history
  - Integration with Rust backend via Tauri IPC

### Backend IPC Commands

Communication between frontend and backend happens via Tauri commands:

```rust
// Core commands in src-tauri/src/lib.rs
read_srt(file_path: String) -> SRTFile
write_srt(file_path: String, entries: Vec<SubtitleEntry>) -> ()
read_audio_file(file_path: String) -> String  // Base64 encoded
generate_audio_waveform(file_path: String) -> Vec<f32>  // With progress callback
greet(name: String) -> String  // Example command
```

### Audio Processing Pipeline

1. **Audio Reading**: `read_audio_file()` decodes multiple formats using Symphonia
2. **Waveform Generation**:
   - Multi-channel audio converted to mono
   - Downsampling with peak normalization
   - Real-time progress updates (0-100%)
   - Asynchronous processing in background thread

### Type System

Key TypeScript types in `src/types/subtitle.ts`:
- `SubtitleEntry`: ID, start/end times, text content
- `TimeStamp`: SRT format (HH:MM:SS,mmm)
- `SRTFile`: File path, encoding, entries array
- `TimeConflict`: Overlap detection results
- `SearchResult`: Match location and context

## Development Workflow

### Adding a New Feature

1. **Frontend Logic**: Update `src/stores/subtitle.ts` (Pinia store)
2. **Backend Support**: If file I/O or audio processing needed, add Rust command in `src-tauri/src/lib.rs`
3. **UI Implementation**: Create Vue component in `src/components/` or update `EditorPage.vue`
4. **Testing**: Add tests in `tests/unit/`
5. **Type Safety**: Update types in `src/types/subtitle.ts` if needed

### Working with Rust Backend

- **File Location**: `src-tauri/src/`
- **SRT Parsing**: `srt_parser.rs` - add parsing logic here
- **Audio Processing**: `waveform_generator.rs` - handles codec support via Symphonia
- **Async Operations**: Use Tokio for background tasks
- **Error Handling**: Return `Result` types; Tauri serializes as `{ status: "error", message }`

### Testing

- Unit tests in `tests/unit/`
- Vitest configuration includes auto-import and Pinia setup
- Coverage threshold: 10% (lines/branches/statements)
- Run single tests: `pnpm vitest tests/unit/specific.test.ts`

## Code Quality Standards

### Linting & Formatting
- **ESLint**: Enforces Vue 3 + TypeScript best practices
  - `no-var`, `no-console` rules configured
  - Security plugin enabled
  - Vitest plugin for test globals
- **Prettier**: Formatting standard
  - Semi-colons: disabled
  - Quotes: single
  - Tab width: 2
  - Print width: 120
- **Biome**: Alternative formatter with import organization

### TypeScript Configuration
- **Strict mode** enabled
- **Separate configs** for app, build tools, and testing
- Type checking runs before production builds

### VS Code Setup
See `.vscode/extensions.json` for recommended extensions:
- Volar (Vue)
- Tauri tools
- Rust Analyzer
- ESLint, Prettier

## Performance Notes

### Waveform Generation
- Processed asynchronously in background thread with progress callbacks
- Downsampling applied to prevent UI freezing
- Peak normalization for visual consistency

### State Management
- History limited to 100 entries to control memory usage
- Computed properties used for derived state
- Search indexing for fast lookups

### Build Optimization
- Rust: LTO enabled, size optimization (`opt-level = "s"`)
- Frontend: Tree-shaking, code splitting, minification in production

## Platform-Specific Details

### macOS
- Target: Safari 16+
- Menu: App + File + Edit with Cmd keyboard shortcuts
- Window: 1200x800

### Windows
- Target: Chrome 107+
- Menu: File + Edit with Ctrl keyboard shortcuts
- App icons included

### Linux
- Supported via Tauri
- Same feature set as Windows

## Recent Development Focus

Based on recent commits, active development areas include:
- Keyboard navigation in subtitle lists and search
- Search result highlighting
- Multi-select and batch move operations
- Waveform generation progress tracking

## Common Development Tasks

### Adding a Keyboard Shortcut
1. Define action in `src/stores/subtitle.ts`
2. Add menu item in `src-tauri/src/main.rs` (menu definitions)
3. Listen for menu event in `EditorPage.vue` or relevant component
4. Call store action

### Batch Processing Subtitles
- Use `src/stores/subtitle.ts` batch methods (e.g., `removeHtmlTags`, `removePunctuation`)
- These modify all entries and create a single undo/redo action
- Update history automatically via Pinia mutations

### Extending SRT Parser
- Modify `src-tauri/src/srt_parser.rs`
- Update `SubtitleEntry` type if adding new fields
- Ensure backward compatibility with existing SRT files

## File I/O & Security

- All file operations go through Tauri IPC with proper error handling
- File dialog plugin handles native file pickers
- Audio file reading supports multiple formats via Symphonia
- Base64 encoding used for binary audio data transfer

## Debugging

### Frontend
- Vue DevTools available via `pnpm dev:full`
- Browser DevTools accessible in development mode
- Console errors logged with context

### Backend (Rust)
- Run with `RUST_BACKTRACE=1` (default in dev)
- Use VS Code "Debug Tauri" configuration for breakpoint debugging
- Print debugging with `println!` or `eprintln!`

## Versioning & Release Process

1. Update version in `Cargo.toml` and `package.json`
2. Run `pnpm bump X.Y.Z` to automate version bumping
3. Run `pnpm check` to update `Cargo.lock`
4. Tag commit with `vX.Y.Z`
5. GitHub Actions automatically creates draft release
6. Edit release notes and publish

## Configuration Files Reference

| File | Purpose |
|------|---------|
| `tauri.conf.json` | App window, security, capabilities |
| `vite.config.ts` | Build config, dev server, plugins |
| `vitest.config.ts` | Test runner config, coverage settings |
| `tsconfig.*.json` | TypeScript compiler options |
| `.prettierrc.json` | Code formatting rules |
| `biome.json` | Alternative formatter with linting |
| `.editorconfig` | Cross-editor formatting standards |

## Notes for Future Development

- Current test coverage is minimal (10% threshold) - expand test suite for new features
- Consider performance implications when adding features that modify large subtitle files
- Audio format support is comprehensive (MP3, WAV, AAC, FLAC, OGG) via Symphonia
- UI framework (Element Plus + Tailwind) allows rapid component development
- Auto-import plugin reduces boilerplate - leverage it for new components

# 用户自定义 

- 不要执行 pnpm tauri dev，我自己会在终端执行
- 我开发的桌面端应用，需要支持 MacOS 和 Windows 
- 关于不同操作系统的快捷键，你需要支持一下。