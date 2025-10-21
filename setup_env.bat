@echo off
echo Setting up Visual Studio environment...

REM Try to find and run vcvars64.bat
if exist "C:\Program Files\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" (
    call "C:\Program Files\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
    echo Visual Studio 2022 Build Tools environment set up
) else if exist "C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvars64.bat" (
    call "C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
    echo Visual Studio 2019 Build Tools environment set up
) else (
    echo Visual Studio Build Tools not found in expected locations
    echo Please install Visual Studio Build Tools 2022 with C++ workload
    pause
    exit /b 1
)

echo Setting up Rust environment...
set PATH=%PATH%;C:\Program Files\Rust stable MSVC 1.90\bin

echo Running HFT Bot...
cargo run

pause

