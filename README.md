# Reverse-engineered driver for the DSB012 fitness tracker

The DSB012 tracker (Also called `SATB 4.0 A1`) is a cheap fitness tracker that requires a smartphone app to read any data. The app was removed from the Play Store and the Apple App Store, and the apk file only supports the armv7 architecture and does not work on anything newer than Android 7. This project aims to get it functional on modern devices with a non-clunky UI, and potentially integrate it into other services.

# Why?

Because it's a fun lesson in reverse engineering, and because I have one and hate e-waste.

## Why Rust?

Rust lets you build blazing fast and reliable programs easily. Besides, you can use WebAssembly to integrate it into a web app,
