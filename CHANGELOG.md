# Changelog

What a desktop-kit version changes for the people using the apps. Each tag's
notes go into every app's `## [Unreleased]` on their own: the tag workflow
opens a pull request in each app that moves it to the new tag and adds these
notes, with `{app}` replaced by the app's name. A note an app already has is
not added twice.

- Every version that changes the kit gets a section here, `## [x.y.z]`, in
  the pull request that bumps `version` in Cargo.toml. CI refuses one without.
- Group the bullets under `### Added`, `### Changed`, `### Fixed` or
  `### Removed`, written for someone using the app, as in the apps' own
  changelogs: what they will see, never a module or function name.
- No change a user could notice? Write the heading and nothing under it: the
  apps' pull requests then take the `no-notes` label.

## [0.1.5]

### Added

- Install & restart, on the banner that says a new version is out: one click
  downloads it, checks it, puts it in place of the version you have and opens
  it again, on macOS, Windows and Linux. No disk image to open, drag and throw
  away, no setup to click through. If an update can't finish, {app} says why
  the next time it opens. Where it can't replace itself (an app in a folder you
  can't change), the banner offers the download instead.
