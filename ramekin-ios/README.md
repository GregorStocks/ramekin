# Ramekin iOS

iOS app and Safari Share Extension for saving recipes to Ramekin.

## Requirements

- macOS with Xcode 15+
- iOS 16+ device or simulator
- [XcodeGen](https://github.com/yonaskolb/XcodeGen) (`brew install xcodegen`)

## Setup

1. Install XcodeGen if you haven't already:
   ```bash
   brew install xcodegen
   ```

2. Generate the Xcode project:
   ```bash
   cd ramekin-ios
   xcodegen generate
   ```

3. Open the project in Xcode:
   ```bash
   open Ramekin.xcodeproj
   ```

4. Select your development team in Xcode:
   - Click on the project in the navigator
   - Select the "Ramekin" target
   - Go to "Signing & Capabilities"
   - Select your team from the dropdown
   - Repeat for "RamekinShareExtension" target

5. **Important**: Update the App Group identifier if needed:
   - The default is `group.com.ramekin.app`
   - If you need to change it, update it in:
     - `Ramekin/Ramekin.entitlements`
     - `RamekinShareExtension/RamekinShareExtension.entitlements`
     - `Shared/KeychainHelper.swift` (the `accessGroup` constant)

6. Connect your iPhone and run!

## Usage

1. **First time setup**: Open the Ramekin app and sign in with your server URL and credentials

2. **Saving recipes**:
   - Open any recipe webpage in Safari
   - Tap the Share button (square with arrow)
   - Scroll to find "Ramekin" (or tap "More" to find it)
   - The recipe will be sent to your Ramekin server for processing

## Architecture

```
ramekin-ios/
├── Ramekin/                    # Main iOS app
│   ├── RamekinApp.swift        # App entry point
│   ├── LoginView.swift         # Login screen
│   ├── SettingsView.swift      # Settings/status screen
│   └── Assets.xcassets/        # App icons and colors
├── RamekinShareExtension/      # Safari Share Extension
│   ├── ShareViewController.swift
│   └── Info.plist
├── Shared/                     # Shared code between app and extension
│   ├── KeychainHelper.swift    # Secure credential storage
│   └── RamekinAPI.swift        # API client
└── project.yml                 # XcodeGen project specification
```

## Troubleshooting

### "Not Signed In" in Share Extension
The main app and Share Extension share credentials via an App Group. Make sure:
- Both targets have the same App Group enabled
- You've signed in via the main Ramekin app first

### Share Extension doesn't appear
- After installing, you may need to enable the extension manually
- Go to Settings → Safari → Extensions → enable Ramekin
- Or in the Share sheet, tap "Edit Actions" and add Ramekin

### App expires after 7 days
With a free Apple Developer account, apps expire after 7 days. Either:
- Re-run from Xcode to reinstall
- Get a paid Apple Developer account ($99/year) for 1-year validity
