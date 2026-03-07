# Changelog - GoldSrc Reverb Plugin

All notable changes to this project will be documented in this file.

## [0.3.6] - 2026-03-07
### Added
- **Recursive Preset Loading:** The plugin now scans for user presets recursively within subdirectories.
- **Improved Preset Labels:** Preset names now include their directory path for better organization (e.g., "Subdir / Preset").
- **Window Size Persistence:** Added a new parameter to track and persist the GUI window size across sessions.
### Fixed
- **UI Logic:** Refactored window size selection and preset indexing for better reliability and parameter synchronization.

## [0.3.5] - 2026-03-07
### Added
- **Preset Categories:** Introduced 'User' and 'Factory' categories in the preset loader to better organize saved settings.

## [0.3.3] - 2026-03-07
### Added
- **UI Indicators:** Added an asterisk (`*`) next to the preset name when parameters have been modified, providing visual feedback for unsaved changes.
### Fixed
- **Bug Fix:** Fixed a bug where the displayed preset name wouldn't update correctly under certain conditions.

## [0.3.2] - 2026-03-06
### Fixed
- **Maintenance:** Cleaned up unused functions and performed minor code refactoring.

## [0.3.1] - 2026-03-06
### Added
- **Testing:** Added unit tests specifically for the preset management system to ensure reliability.

## [0.3.0] - 2026-03-06
### Added
- **User Preset Storage:** Added the ability for users to save and load their own presets via the GUI.
- **Dependencies:** Integrated `rfd` (Native File Dialogs) and `pollster` to handle asynchronous file operations for saving/loading.

## [0.2.0] - 2026-03-05
### Added
- **GUI Dropdown:** Implemented a new dropdown menu for quick preset selection.
- **Serialization:** Integrated `serde` and `serde_json` for preset data handling.
### Fixed
- **Parameter Sync:** Fixed an issue where changing a preset wouldn't correctly update the internal DSP parameters as intended.

## [0.1.2] - 2026-03-05
### Added
- **GUI Initialization:** Initial setup of the Vizia-based GUI (`nih_plug_vizia`).
### Fixed
- **Compatibility Fix:** Patched `baseview` (via a custom fork) to prevent a critical Windows bug where the UI would freeze when hovering over the window border during modal interactions.
- **Parameter Legibility:** Updated host parameters to be human-legible in generic DAW views.
- **DSP Update:** Linked to `goldsrc_dsp` revision `9695b98`, which added the ability to toggle the clipping stage.

## [0.1.1] - 2026-03-04
### Added
- **RNG Control:** Added a dedicated **RNG Seed** parameter, allowing users to control the randomness of the reverb texture.
- **DSP Update:** Linked to `goldsrc_dsp` revision `1c25fcb`, adding the seed setter method.

## [0.1.0] - 2026-03-03
### Added
- **Initial Release:** Base plugin implementation using `goldsrc_dsp` v0.2.
- **Core Functionality:** Basic reverb signal flow using the original GoldSrc (Half-Life) algorithm.
