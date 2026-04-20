# Changelog

## [Unreleased]

## [v0.2.0] - 2026-04-20

### Added

- `activate(app_name)` and `uninstall(app_name)`.

### Changed

- Refined the installer API around `/Applications/<app name>`.
- Changed `install(path, force)` to non-destructive `install(source)`.
- Improved activation diagnostics and session startup using the active registered appex.
- Strengthened uninstall cleanup for LaunchServices, PlugInKit, `Application Scripts`, and `Containers`.

## [v0.1.0] - 2025-11-08

- Initial release of **fskit-rs**. See details in the [README](README.md).

[Unreleased]: ../../compare/v0.2.0...HEAD
[0.2.0]: ../../releases/tag/v0.2.0
[0.1.0]: ../../releases/tag/v0.1.0
