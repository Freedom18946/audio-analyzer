# Audio Analyzer

This project consists of two main parts:
1.  A **Rust application** that serves as the main entry point.
2.  A **Python GUI application** for audio analysis, which is packaged as a standalone executable and called by the Rust application.

## Project Overview

The primary goal is to provide a robust audio analysis tool. The Rust application is responsible for launching and managing the Python-based analysis GUI. The Python application uses the `ffmpeg` binary for powerful audio file processing.

This hybrid structure leverages Rust's performance for the main application logic and Python's rich ecosystem for data analysis and GUI development.

## Project Structure

```
/
├── Cargo.toml          # Rust project configuration
├── src/main.rs         # Rust source code (Main entry point)
│
├── scripts/            # Python source code for the analyzer GUI
│   └── audio-analyzer.py # Main Python application logic and GUI
│
├── requirements.txt    # Python dependencies (for development)
├── audio-analyzer.spec # PyInstaller configuration file
│
├── resources/          # External binaries called by the application
│   ├── audio-analyzer  # The packaged Python application (executable)
│   └── ffmpeg          # Pre-compiled ffmpeg binary for audio processing
│
├── archive/            # Backups of older source code
│   ├── 3.0.rs
│   └── ana_aud.py
│
├── build/              # Intermediate PyInstaller build files (ignored)
├── dist/               # PyInstaller output directory (ignored)
└── target/             # Rust compiler output directory (ignored)
```

### Key Components

*   **Rust Application**:
    *   The main executable of the project, built from the code in `src/main.rs`.
    *   It is responsible for bundling all necessary resources (`ffmpeg` and the Python app) and launching the Python GUI.

*   **Python Audio Analyzer (Packaged)**:
    *   The file `resources/audio-analyzer` is **not source code**. It is a standalone executable built from the Python script at `scripts/audio-analyzer.py` using PyInstaller.
    *   This executable handles all the GUI and audio analysis logic.

*   **`ffmpeg`**:
    *   The `ffmpeg` binary in `resources/` is a critical, pre-compiled dependency.
    *   The Python application calls `ffmpeg` to ensure compatibility with a wide range of audio formats and to perform complex audio manipulations, providing more robust functionality than standard Python libraries alone.

## How to Build the Project

Building this project is a two-step process.

### Step 1: Build the Python Executable

First, you must build the Python script into a standalone executable.

1.  **Install Python dependencies**:
    ```bash
    pip install -r requirements.txt
    ```

2.  **Run PyInstaller**:
    Use the following command to create the executable. This command packages the script into a single file named `audio-analyzer`.
    ```bash
    pyinstaller -F --name audio-analyzer --clean scripts/audio-analyzer.py
    ```

3.  **Move the Executable**:
    After the build succeeds, you must manually move the executable from the `dist/` directory to the `resources/` directory.
    ```bash
    # On macOS or Linux
    mv dist/audio-analyzer resources/audio-analyzer

    # On Windows
    # move dist\audio-analyzer.exe resources\audio-analyzer.exe
    ```
    **Note**: The `resources/audio-analyzer` file must be present for the main Rust application to work.

### Step 2: Build the Rust Application

Once the Python executable is in place, you can build the main Rust application.

```bash
cargo build --release
```

The final, complete application will be located in `target/release/`. This single executable can be run, and it will launch the Python GUI as needed.
