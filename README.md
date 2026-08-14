# DefaultSynth

`SYNTH.png`의 디자인을 목표로 하는 폴리포닉 신디사이저입니다. 하나의 Rust 코드베이스에서
**CLAP과 VST3를 동시에** 빌드합니다.

## 라이선스 (중요)

NIH-plug 자체는 ISC지만 **`nih_export_vst3!`가 쓰는 VST3 바인딩은 GPLv3**입니다.
따라서 VST3 빌드를 배포하는 순간 이 플러그인 전체가 GPLv3 적용 대상이 됩니다.
**CLAP 빌드에는 이 제약이 없습니다** (CLAP은 MIT).

클로즈드 소스로 판매할 계획이 생기면 VST3 경로를 JUCE + Steinberg 상용 라이선스로
교체해야 하며, `crates/ds-dsp`는 프레임워크 의존성이 없으므로 그대로 재사용할 수 있습니다.

## 빌드

Rust stable과 (Windows 기준) MSVC 툴체인이 필요합니다.

```bash
cargo xtask bundle defaultsynth --release
```

산출물은 `target/bundled/`에 생성됩니다.

- `DefaultSynth.clap`
- `DefaultSynth.vst3`

Windows에서는 `build.bat`을 쓰면 빌드 캐시를 작업공간 밖(`C:\tmp\defaultsynth-target`)으로
빼줍니다. OneDrive/Desktop 인덱서나 백신이 Rust의 증분 오브젝트를 잠깐씩 잠가
`os error 32`로 빌드가 깨지는 환경에서 필요합니다. `CARGO_TARGET_DIR`을 미리 설정하면
그 값이 우선합니다.

### 설치

호스트는 정해진 폴더만 스캔합니다. 아래에 통째로 복사하세요. `.vst3`는 파일이 아니라
**폴더**이므로 폴더째 옮겨야 합니다.

**전체 사용자** (관리자 권한 필요)

| 포맷 | 경로 |
| --- | --- |
| CLAP | `C:\Program Files\Common Files\CLAP\` |
| VST3 | `C:\Program Files\Common Files\VST3\` |

**현재 사용자만** (권한 불필요, 권장)

| 포맷 | 경로 |
| --- | --- |
| CLAP | `%LOCALAPPDATA%\Programs\Common\CLAP\` |
| VST3 | `%LOCALAPPDATA%\Programs\Common\VST3\` |

```bat
mkdir "%LOCALAPPDATA%\Programs\Common\CLAP" 2>nul
mkdir "%LOCALAPPDATA%\Programs\Common\VST3" 2>nul
copy /Y "%CARGO_TARGET_DIR%\bundled\DefaultSynth.clap" "%LOCALAPPDATA%\Programs\Common\CLAP\"
xcopy /E /I /Y "%CARGO_TARGET_DIR%\bundled\DefaultSynth.vst3" "%LOCALAPPDATA%\Programs\Common\VST3\DefaultSynth.vst3"
```

복사한 뒤 호스트에서 플러그인 폴더를 다시 스캔해야 목록에 나타납니다.

macOS는 `~/Library/Audio/Plug-Ins/CLAP/`과 `~/Library/Audio/Plug-Ins/VST3/`,
Linux는 `~/.clap/`과 `~/.vst3/`입니다.

## 호스트 없이 실행하기

DAW를 설치하지 않아도 스탠드얼론으로 바로 연주할 수 있습니다.

```bat
run.bat
```

시스템 오디오 출력과 MIDI 입력에 직접 연결되며 플러그인과 같은 에디터가 열립니다.

```bat
run.bat --midi-input ""          :: 사용 가능한 MIDI 입력 나열
run.bat --midi-input "MPK mini"  :: 키보드 연결
run.bat --output-device ""       :: 사용 가능한 출력 장치 나열
```

**버퍼 크기 주의.** Windows 공유 모드 WASAPI는 장치가 정한 주기를 그대로 넘기는데,
NIH-plug의 스탠드얼론 백엔드는 요청값과 다르면 오디오 스레드에서 패닉합니다.
`Received 1056 samples, while the configured buffer size is 512` 같은 메시지가 나오면
그 숫자를 그대로 넘기세요.

```bat
set DEFAULTSYNTH_PERIOD=1056
run.bat
```

`run.bat`은 개발 머신에서 확인된 1056을 기본값으로 씁니다. 장치마다 다릅니다.

### 검증

```bash
cargo test                                   # DSP 53개 + 플러그인 15개
clap-validator validate <경로>/DefaultSynth.clap
```

### 빌드 캐시 정리

Rust 빌드 캐시는 금방 몇 기가씩 불어납니다. 전부 재생성되는 파생물이므로
언제든 지워도 됩니다. 단, Cargo나 실행 중인 플러그인이 파일을 잡고 있으면
삭제가 실패하니 먼저 닫으세요.

| 경로 | 내용 | 지워도 되나 |
| --- | --- | --- |
| `C:\tmp\defaultsynth-target\debug` | `cargo test`/`cargo check` 산출물. 가장 크게 자랍니다 | 예, 다음 테스트에서 재생성 |
| `C:\tmp\defaultsynth-target\release` | 릴리스 빌드와 스탠드얼론 실행 파일 | 예, 다음 빌드에서 재생성(수 분 소요) |
| `C:\tmp\defaultsynth-target\bundled` | 완성된 `.clap` / `.vst3` | 지우면 다시 번들해야 합니다 |
| `%USERPROFILE%\.cargo\registry` | 내려받은 크레이트 소스와 압축 파일 | 예, 다시 받습니다(네트워크 필요) |
| `%USERPROFILE%\.cargo\git` | git 의존성 체크아웃(NIH-plug, VIZIA) | 예, 다시 받습니다 |

가장 큰 것만 빠르게 비우려면:

```bat
rmdir /S /Q "C:\tmp\defaultsynth-target\debug"
```

전체를 비우려면 `C:\tmp\defaultsynth-target` 폴더째 지우면 됩니다. 다음
`build.bat` 실행 때 처음부터 다시 만들어집니다.

## 구조

의도적으로 두 크레이트로 나눴습니다.

- **`crates/ds-dsp`** — 합성 코어. 플러그인 프레임워크 의존성이 **전혀 없습니다**.
  샘플과 노트만 알고 VST3/CLAP/GUI는 모르므로, 평범한 `cargo test`로 검증되고
  플러그인 밖에서도 재사용할 수 있습니다.
  - `oscillator.rs` — PolyBLEP 안티에일리어싱 오실레이터, 유니즌 스택
  - `envelope.rs` — AHDSR (UI의 ATTACK/HOLD/DECAY/SUSTAIN/RELEASE)
  - `filter.rs` — ZDF 상태변수 필터 (LP/HP/BP/Notch)
  - `noise.rs` — White / Pink / Brown
  - `voice.rs` — 오실레이터 3개 + 노이즈 + 필터 2개 + 앰프 엔벨로프
  - `engine.rs` — 보이스 할당, 스틸링, Poly/Mono/Legato, 포르타멘토
- **`crates/defaultsynth`** — nih-plug 래퍼. 파라미터 정의, MIDI 처리, VIZIA 에디터.
- **`xtask`** — `.clap` / `.vst3` 번들 패키징.

## 현재 상태

**동작하는 것**

- CLAP·VST3 양쪽 번들 생성, 호스트 로드, MIDI 입력으로 소리 남
- 오실레이터 3개(파형·옥타브·파인·유니즌·디튠·블렌드·워프·위상·팬·레벨)
- 노이즈 3색, 필터 2개(모드·컷오프·레조넌스·엔벨로프량·키트랙), 필터 A/B 라우팅
- 앰프/필터 엔벨로프, Poly/Mono/Legato + 포르타멘토 + 벨로시티 커브
- **LFO 4개**: 직접 그리는 커스텀 파형 + 내장 6종(사인·삼각·업/다운 톱니·사각·S&H),
  딜레이·라이즈, Trigger/Free/Envelope 트리거 모드,
  호스트 BPM 동기(1/128 ~ 4Bar, 셋잇단·점음표)
- **모듈레이션 매트릭스 8슬롯**: 소스 11종(LFO 1–4 / 앰프·필터·모드 엔벨로프 /
  벨로시티 / 키트랙 / 모드휠), 데스티네이션 13종(피치·필터 A/B 컷오프·레조넌스·
  음량·팬·OSC A/B/C 워프·레벨·디튠), 바이폴라 양 조절
- 모든 파라미터가 호스트에 노출·오토메이션 가능

### LFO 파형 그리기

LFO 패널의 창은 세럼처럼 직접 그리는 에디터입니다.

| 조작 | 결과 |
| --- | --- |
| 빈 곳 더블클릭 | 그 자리에 포인트 추가 |
| 포인트 더블클릭 | 포인트 제거 (양 끝은 제거 불가) |
| 포인트 드래그 | 이동. 좌우 이웃을 넘어가지 않음 |
| 선분 위에 마우스 | 가운데에 곡률 핸들이 뜸 |
| 핸들 드래그 | 그 구간의 곡선을 휘게 함 |
| 핸들 더블클릭 | 그 구간을 직선으로 되돌림 |
| 우클릭 | 내장 파형을 순서대로 넘김 |

새 LFO는 가운데가 솟은 `^` 모양(포인트 3개)에서 시작하고, 처음부터 편집 가능한
상태입니다.

**양 끝은 항상 같은 값입니다.** LFO는 순환하므로 양 끝이 다르면 한 바퀴 돌 때마다
계단이 생깁니다. 그래서 끝점은 x가 0과 1에 고정되고, 어느 쪽을 끌어도 둘이 함께
움직입니다.

내장 파형이 선택된 상태에서 창을 편집하면 그 파형을 포인트로 옮겨 담아 커스텀으로
전환합니다. 내장 파형은 막다른 길이 아니라 출발점입니다. 디자인의 LFO 패널에는 긴
선택 박스가 하나뿐이고 그 라벨이 TRIG이라, 디자인에 없는 박스를 새로 만드는 대신
창 자체가 파형까지 맡습니다.

그린 곡선은 파라미터가 아니라 세션에 저장되는 상태(`lfo1curve` ~ `lfo4curve`)입니다.
길이가 변하는 구조라 호스트가 오토메이션할 수 있는 float 목록으로 정직하게 표현할
방법이 없기 때문입니다.

매트릭스는 아직 전용 페이지가 없어서 호스트의 기본 파라미터 패널에서 조작합니다
(`Mod Matrix` 그룹의 Slot 1–8). MATRIX 페이지를 그리면 그 UI로 옮겨갑니다.

**OSC 탭은 화면상 완성되었습니다.** 상단 페이지 탭과 프리셋 바, 오실레이터 3개,
ENV 1–3, LFO 1–4, NOISE, FILTER A/B, VOICING이 모두 그려져 있고 모든 컨트롤이 실제
파라미터에 연결되어 호스트에서 오토메이션·저장됩니다.

**그려졌지만 아직 소리에 닿지 않는 것**

비주얼을 먼저 맞추는 방향이라, 아래 컨트롤은 화면과 파라미터만 있고 DSP가 아직 읽지
않습니다. 호스트에서 움직이고 저장할 수는 있지만 소리는 바뀌지 않습니다.

- OSC의 DIR 출력과 모드 소스(FM / RING / SYNC)
- 필터의 입력 선택 A/B/C/N, DRIVE / FREQ / MIX, PAN
- 노이즈의 PITCH와 KEYTRACK
- LFO의 ANCH(트랜스포트 위치에 위상 고정)
- ENV 3은 고정된 목적지가 없어서, 매트릭스에서 Mod Env로 라우팅해야 소리에 닿음
- VOICING의 NOTE 커브
- 상단 EFFECT / MATRIX / GLOBAL 페이지와 프리셋 브라우저
- 웨이브테이블 (현재는 기본 파형 4종)

## clap-validator 결과

44개 중 31개 통과, 9개 스킵, **4개 실패**.

실패 4개는 `state-reproducibility-{basic,binary,buffered}`와 `state-invalid-random`입니다.
이는 이 프로젝트의 결함이 아니라 **nih-plug과 clap-validator 사이의 버전 불일치**입니다.
nih-plug 공식 `gain` 예제를 같은 검사기로 돌리면 동일한 4개가 동일한 방식으로 실패합니다
(대조군: 28 passed / 4 failed). 상태 저장·복원 자체는 호스트에서 정상 동작합니다.

검사기가 잡아낸 **진짜 결함 2개는 수정했습니다**.

- `Cutoff`가 `"1.1 kHz Hz"`를 출력해 되읽기 실패 — `v2s_f32_hz_then_khz`가 이미 단위를
  포함하는데 `with_unit`을 중복으로 붙인 것이 원인
- 파라미터 극단값에서 NaN 출력 — 마스터 게인의 최솟값이 0인데 로그 스무딩을 걸어
  로그 영역 보간이 발산. 최솟값을 -60 dB로 올려 해결
