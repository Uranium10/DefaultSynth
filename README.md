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

CLAP은 `%COMMONPROGRAMFILES%\CLAP\`, VST3는 `%COMMONPROGRAMFILES%\VST3\`에 복사하면
호스트가 인식합니다.

### 검증

```bash
cargo test                                   # DSP 48개 + 파라미터 4개
clap-validator validate <경로>/DefaultSynth.clap
```

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
- 74개 파라미터 전부 호스트에 노출·오토메이션 가능

**아직 없는 것** (디자인에는 있으나 미구현)

- 웨이브테이블 (현재는 기본 파형 4종)
- LFO, 모드 매트릭스, 이펙트 랙, 프리셋 브라우저
- ENV/LFO 개수 확장(+ 버튼), FM 라우팅
- 커스텀 위젯: 원형 노브, 파형 디스플레이, 엔벨로프·LFO 곡선 에디터
  (현재 에디터는 모든 컨트롤이 실제 파라미터에 연결된 레이아웃 골격입니다)

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
