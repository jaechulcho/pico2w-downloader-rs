# pico2w-downloader-rs (Pico 2 W Rust Downloader)

Windows 및 Linux에서 사용 가능한 Pico 2 W 전용 펌웨어 업로드 유틸리티입니다.
`pico2w-bootloader-rs`와 연동되어 안정적인 펌웨어 업데이트(DFU)를 수행합니다.

## 주요 기능

- **크로스 플랫폼**: Rust로 구현되어 Windows와 Linux 환경을 모두 지원합니다.
- **Hex/Bin 자동 파싱**: Intel HEX 파일과 raw binary 파일을 모두 지원합니다.
- **자동 원격 업데이트**: `--reboot` 플래그를 통해 실행 중인 펌웨어를 자동으로 재시작하고 업데이트 모드로 진입시킵니다.
- **무결성 검증**: 전송할 데이터의 CRC32를 계산하여 부트로더에 전달합니다.

## 사용 방법

### 1. 빌드
```bash
cargo build --release
```

### 2. 실행 명령어

#### 자동 리셋 및 업데이트 (추천)
현재 Pico에서 `pico2w-shell-rs`가 실행 중인 경우, `--reboot` 옵션을 사용하여 한 번에 업데이트를 완료할 수 있습니다.
```bash
# Windows
cargo run --release -- COM3 firmware.hex --reboot

# Linux
cargo run --release -- /dev/ttyACM0 firmware.bin --reboot
```

#### 수동 업데이트
Pico가 이미 업데이트 모드(부트로더 대기 중)인 경우:
```bash
cargo run --release -- <PORT> <FILE>
```

## 주요 옵션

- `<PORT>`: 시리얼 포트 이름 (예: `COM3`, `/dev/ttyACM0`)
- `<FILE>`: 업로드할 파일 (`.hex` 또는 `.bin`)
- `-r, --reboot`: 업데이트 전 `reboot` 명령어를 보내 소프트웨어 재시작을 유도합니다.
- `-b, --baud`: 통신 속도 (기본값: 115200)

## 라이선스

이 프로젝트는 MIT 및 Apache-2.0 라이선스 하에 배포됩니다.
