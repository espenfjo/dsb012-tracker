# Reverse-engineered driver for the DSB012 fitness tracker

The DSB012 tracker (Also called `SATB 4.0 A1`) is a cheap fitness tracker that requires a smartphone app to read any data. The app was removed from the Play Store and the Apple App Store, and the apk file only supports the armv7 architecture and does not work on anything newer than Android 7. This project aims to get it functional on modern devices with a non-clunky UI, and potentially integrate it into other services.

# State of the project

Feature | Status
------- | ------
Send / recv packets | ok
Read battery and fw revision | ok
Pull raw data files | ok
Parse received data | in progress
Analyze fitness date | planned
Export csv | planned
Plot nice graphs | planned
Integration with other services | maybe
Goal setting and guidance | not planned
Make a cool PWA | maybe

# Technical details of the device

Warning: This is very incomplete!

Tracker supports:
- Step counting
- Sedentary time
- Sleep time
- Derived amount of calories burned

## Protocol

- Uses BLE characteristics, one to send commands to the device, one to recieve data
- Uses 20-byte commands and responses
- First byte is constant
- Second byte indicates command / response type
- Last two bytes are a CRC checksum

### Connection Sequence

- Pair with device
- Request battery level, fw revision
- Synchronize time between device and tracker
- Request data info
- Request the actual data, takes a few moments to receive it all
- Send data finish, this deletes the stored data from the tracker to free up storage

# Why?

Because it's a fun lesson in reverse engineering, and because I have one and hate e-waste.

## Why Rust?

Rust lets you build blazing fast and reliable programs easily. Besides, you can use WebAssembly to integrate it into a web app,
