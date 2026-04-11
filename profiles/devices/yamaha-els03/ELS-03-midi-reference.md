# STAGEA ELS-03 MIDIリファレンス

対象機種: ELS-03G / ELS-03X / ELS-03XR / ELS-03XF  
© 2026 Yamaha Corporation — 2026年4月発行 YJ-B0

---

## 略語

| 略語 | 意味 |
|------|------|
| UK | 上鍵盤 |
| LK | 下鍵盤 |
| PK | ペダル鍵盤 |
| Lead1 | キーボードアサイン(リード1) |
| KBP | キーボードパーカッション |
| Ctrl | コントロール |

---

## チャンネルメッセージ＆リアルタイムメッセージ

> \*送信チャンネルは[UTILITY]→[MIDI]で設定  
> Lead1パートはMIDI設定でLead1をExternalに設定したときだけ4chで受信する

### 受信チャンネル割り当て

| パート | チャンネル |
|--------|-----------|
| UK | 1ch |
| LK | 2ch |
| PK | 3ch |
| Lead1 | (4ch) ※External設定時のみ |
| XG | 5〜14ch |
| KBP | 15ch |
| Ctrl | 16ch |

### Note Off

| Status | 1st Data (Data) | 1st Data (Parameter) | 2nd Data (Data) | 2nd Data (Parameter) | MIDI受信 UK/LK/PK/Lead1/XG/KBP/Ctrl | MIDI送信 UK/LK/PK/Lead1/XG/KBP/Ctrl | MDR 再生/録音 |
|--------|-----------------|---------------------|-----------------|---------------------|--------------------------------------|--------------------------------------|---------------|
| 8nH (n:Channel) | 00-7F | Key Number | 00-7F | Velocity | 1ch/2ch/3ch/(4ch)/5-14ch/15ch/× | ×/×/×/×/×/×/× | 〇/× |

### Note On

| Status | 1st Data (Data) | 1st Data (Parameter) | 2nd Data (Data) | 2nd Data (Parameter) | MIDI受信 UK/LK/PK/Lead1/XG/KBP/Ctrl | MIDI送信 UK/LK/PK/Lead1/XG/KBP/Ctrl | MDR 再生/録音 |
|--------|-----------------|---------------------|-----------------|---------------------|--------------------------------------|--------------------------------------|---------------|
| 9nH (n:Channel) | 00-7F | Key Number | 00 / 01-7F | Key Off / Key On (Velocity) | 1ch/2ch/3ch/(4ch)/5-14ch/15ch/× | ×/〇(*)/〇(*)/〇(*)/×/×/× | 〇/〇 |

### Control Change

Status: BnH (n:Channel)

| CC# | Parameter | Data | MIDI受信 UK/LK/PK/Lead1/XG/KBP/Ctrl | MIDI送信 UK/LK/PK/Lead1/XG/KBP/Ctrl | MDR 再生/録音 | 備考 |
|-----|-----------|------|--------------------------------------|--------------------------------------|---------------|------|
| 00 | Bank Select MSB | 00-7F | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 01 | Modulation | 00-7F | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 04 | Second Expression | 00-7F | ×/×/×/(4ch)/×/×/16ch | ×/×/×/4ch(*)/×/×/16ch(*) | 〇/〇 | 【受信】*Live Expression Controlの設定により効果が決まる。【送信】*上鍵盤の出力チャンネルを4chにした場合は4chで、それ以外は16chで出力される |
| 05 | Portamento Time | 00-7F | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 06 | Data Entry MSB | 00-7F | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 07 | Main Volume | 00-7F | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 0A | Pan | 00-7F (L64…C…R63) | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 0B | Expression | 00-7F | ×/×/×/×/5-14ch/×/16ch | ×/×/×/×/×/×/〇(*) | 〇/〇 | 送信はExpression PedalにExpression機能がアサインされている時のみ |
| 10 | VA After | 00-7F | 1ch/×/×/×/×/×/× | 〇(*)/×/×/×/×/×/× | 〇/〇 | VA音源にのみ効果有 |
| 20 | Bank Select LSB | 00-7F | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 26 | Data Entry LSB | 00-7F | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 40 | Sustain(Damper) | 00-7F | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 41 | Portamento | 00-3F=OFF / 40-7F=ON | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 42 | Sostenuto | 00-3F=OFF / 40-7F=ON | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 43 | Soft Pedal | 00-3F=OFF / 40-7F=ON | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 47 | Harmonic Content | 00-7F (-64…0…+63) | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 48 | Release Time | 00-7F (-64…0…+63) | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 49 | Attack Time | 00-7F (-64…0…+63) | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 4A | Brightness | 00-7F (-64…0…+63) | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 4B | Decay Time | 00-7F (-64…0…+63) | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 4C | Vibrato Rate | 00-7F (-64…0…+63) | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 4D | Vibrato Depth | 00-7F (-64…0…+63) | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 4E | Vibrato Delay | 00-7F (-64…0…+63) | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 54 | Portamento Control | 00-7F (Key Number) | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 5B | Effect1 Depth (Reverb Send Level) | 00-7F | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 5D | Effect3 Depth (Chorus Send Level) | 00-7F | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 5E | Effect4 Depth (Variation Send Level) | 00-7F | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 60 | RPN Increment | - | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 61 | RPN Decrement | - | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 62 | NRPN LSB | 00-7F | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 63 | NRPN MSB | 00-7F | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 64 | RPN LSB | 00-7F | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |
| 65 | RPN MSB | 00-7F | ×/×/×/×/5-14ch/×/× | ×/×/×/×/×/×/× | XGのみ/× | |

### Channel Mode Message

Status: BnH (n:Channel)

| CC# | Parameter | Data | MIDI受信 UK/LK/PK/Lead1/XG/KBP/Ctrl | MDR 再生/録音 |
|-----|-----------|------|--------------------------------------|---------------|
| 78 | All Sound Off | 00 | ×/×/×/×/5-14ch/×/× | XGのみ/× |
| 79 | Reset All Controllers | 00 | ×/×/×/×/5-14ch/×/× | XGのみ/× |
| 7B | All Note Off | 00 | ×/×/×/×/5-14ch/×/× | XGのみ/× |
| 7C | Omni Off | 00 | ×/×/×/×/5-14ch/×/× | XGのみ/× |
| 7D | Omni On | 00 | ×/×/×/×/5-14ch/×/× | XGのみ/× |
| 7E | Mono | 00-10 | ×/×/×/×/5-14ch/×/× | XGのみ/× |
| 7F | Poly | 00 | ×/×/×/×/5-14ch/×/× | XGのみ/× |

### Program Change

| Status | 1st Data (Data) | 1st Data (Parameter) | MIDI受信 Ctrl | MIDI送信 Ctrl | MDR 再生/録音 |
|--------|-----------------|---------------------|--------------|--------------|---------------|
| CnH (n:Channel) | 00-7F | Registration Memory | 16ch | 16ch | 〇/〇 |
| CnH (n:Channel) | 00-7F | Voice Number (XG) | 5-14ch | × | XGのみ/× |

### Channel After Touch

| Status | 1st Data (Data) | MIDI受信 UK/LK/PK/Lead1 | MIDI送信 UK/LK/PK | MDR 再生/録音 |
|--------|-----------------|------------------------|-------------------|---------------|
| DnH (n:Channel) | 00-7F | 1ch/2ch/3ch/(4ch) | 〇(*)/〇(*)/〇(*) | 〇/〇 |

### Polyphonic After Touch

| Status | 1st Data (Data) | 1st Data (Parameter) | 2nd Data (Data) | MIDI受信 UK/LK/Lead1 | MIDI送信 UK/LK | MDR 再生/録音 |
|--------|-----------------|---------------------|-----------------|---------------------|----------------|---------------|
| AnH (n:Channel) | 00-7F | Key Number | 00-7F | 1ch/2ch/(4ch) | 〇(*)/〇(*) | 〇/〇 |

### Pitch Bend Change

| Status | 1st Data (Data) | 1st Data (Parameter) | 2nd Data (Data) | 2nd Data (Parameter) | MIDI受信 UK/LK/Lead1 | MIDI送信 UK/LK | MDR 再生/録音 |
|--------|-----------------|---------------------|-----------------|---------------------|---------------------|----------------|---------------|
| EnH (n:Channel) | 00-7F | LSB | 00-7F | MSB | 1ch/2ch/(4ch) | 〇(*)/〇(*) | 〇/〇 |

### Realtime Message

| Status | Message | MIDI受信 | MIDI送信 |
|--------|---------|---------|---------|
| F8H | MIDI Clock | ○ | ○ |
| FAH | Start | ○ | ○ |
| FCH | Stop | ○ | ○ |
| FEH | Active Sense | ○ | ○ |
| FFH | System Reset | × | × |

---

## System Exclusive Messages

> \*1 MDR再生: ○=常に再生、C=コントロールパート再生時のみ再生

### Universal Real Time Exclusive Message

#### Master Volume (GM1/GM2)

```
F0,7F,XN,04,01,SS,TT,F7
```

| バイト | 意味 |
|--------|------|
| XN | When N is received N=0-F, whichever is received. X=ignored |
| SS | Volume LSB |
| TT | Volume MSB |

受信:○ / 送信:× / MDR再生:○ / MDR録音:×

#### Master Fine Tuning (GM2)

```
F0,7F,XN,04,03,SS,TT,F7
```

| バイト | 意味 |
|--------|------|
| XN | When N is received N=0-F, whichever is received. X=ignored |
| SS | Fine Tuning LSB |
| TT | Fine Tuning MSB |

受信:○ / 送信:× / MDR再生:○ / MDR録音:×

#### Master Coarse Tuning (GM2)

```
F0,7F,XN,04,04,00,TT,F7
```

| バイト | 意味 |
|--------|------|
| XN | When N is received N=0-F, whichever is received. X=ignored |
| TT | Coarse Tuning MSB |

受信:○ / 送信:× / MDR再生:○ / MDR録音:×

#### Reverb Parameter (GM2)

```
F0,7F,XN,04,05,01,01,01,01,01,PP,VV,…,F7
```

| バイト | 意味 |
|--------|------|
| XN | When N is received N=0-F, whichever is received. X=ignored |
| PP | Parameter to be controlled |
| VV | Value for the Parameter |

受信:○ / 送信:× / MDR再生:○ / MDR録音:×

| PP | Parameter | VV | Description |
|----|-----------|-----|-------------|
| 00 | Reverb Type | 00 | Room S |
| | | 01 | Room M |
| | | 02 | Room L |
| | | 03 | Hall M |
| | | 04 | Hall L (Default) |
| | | 08 | GM Plate |
| 01 | Reverb Time | 00-7F | 0...11.0 [sec]（05-07受信時はHall Lと扱う） |

#### Chorus Parameter (GM2)

```
F0,7F,XN,04,05,01,01,01,01,02,PP,VV,…,F7
```

| バイト | 意味 |
|--------|------|
| XN | When N is received N=0-F, whichever is received. X=ignored |
| PP | Parameter to be controlled |
| VV | Value for the Parameter |

受信:○ / 送信:× / MDR再生:○ / MDR録音:×

| PP | Parameter | VV | Description |
|----|-----------|-----|-------------|
| 00 | Chorus Type | 00 | GM Chorus1 |
| | | 01 | GM Chorus2 |
| | | 02 | GM Chorus3 (Default) |
| | | 03 | GM Chorus4 |
| | | 04 | FB Chorus |
| | | 05 | GM Flanger |
| 01 | Mod Rate | 00-7F | 0...15.5 [Hz] |
| 02 | Mod Depth | 00-7F | 0...127 |
| 03 | Feedback | 00-7F | 0...127 |
| 04 | Send to Reverb | 00-7F | 0...127 |

#### Channel Pressure (Aftertouch) (GM2)

```
F0,7F,XN,09,01,0M,PP,RR,…,F7
```

| バイト | 意味 |
|--------|------|
| XN | When N is received N=0-F, whichever is received. X=ignored |
| 0M | MIDI Channel (00-0F) |
| PP | Controlled Parameter |
| RR | Range |

受信:○ / 送信:× / MDR再生:○ / MDR録音:×

| PP | Parameter | RR (Range) | Description | Default Value |
|----|-----------|-----------|-------------|---------------|
| 00 | Pitch Control | 28-58 | -24...0...+24 [semitones] | 40 |
| 01 | Filter Cutoff Control | 00-7F | -9600...0...+9450 [cents] | 40 |
| 02 | Amplitude Control | 00-7F | -100...0...+100 [%] | 40 |
| 03 | LFO Pitch Depth | 00-7F | 0...127 | 00 |
| 04 | LFO Filter Depth | 00-7F | 0...127 | 00 |
| 05 | LFO Amplitude Depth | 00-7F | 0...127 | 00 |

#### Controller (Control Change) (GM2)

```
F0,7F,XN,09,03,0M,CC,PP,RR,…,F7
```

| バイト | 意味 |
|--------|------|
| XN | When N is received N=0-F, whichever is received. X=ignored |
| 0M | MIDI Channel (00-0F) |
| CC | Controller Number (01-1F, 40-5F) |
| PP | Controlled Parameter |
| RR | Range |

受信:○ / 送信:× / MDR再生:○ / MDR録音:×

| PP | Parameter | RR (Range) | Description | Default Value |
|----|-----------|-----------|-------------|---------------|
| 00 | Pitch Control | 28-58 | -24...0...+24 [semitones] | 40 |
| 01 | Filter Cutoff Control | 00-7F | -9600...0...+9450 [cents] | 40 |
| 02 | Amplitude Control | 00-7F | -100...0...+100 [%] | 40 |
| 03 | LFO Pitch Depth | 00-7F | 0...127 | 00 |
| 04 | LFO Filter Depth | 00-7F | 0...127 | 00 |
| 05 | LFO Amplitude Depth | 00-7F | 0...127 | 00 |

#### Key-Based Instrument Control (GM2)

```
F0,7F,XN,0A,01,0M,KK,CC,VV,…,F7
```

| バイト | 意味 |
|--------|------|
| XN | When N is received N=0-F, whichever is received. X=ignored |
| 0M | MIDI Channel (00-0F) |
| KK | Key Number |
| CC | Controller Number |
| VV | Value |

受信:○ / 送信:× / MDR再生:○ / MDR録音:×

| CC | Parameter | VV (Range) | Description | Default Value |
|----|-----------|-----------|-------------|---------------|
| 07 | Volume | 00-7F | -100...0...+100 [%] | 40 |
| 0A | Pan (Absolute) | 00-7F | L63...C...R63 | (Preset value) |
| 5B | Reverb Send Level (Absolute) | 00-7F | 0...MAX | (Preset value) |
| 5D | Chorus Send Level (Absolute) | 00-7F | 0...MAX | (Preset value) |

---

### Universal Non-Real Time Exclusive Message

#### GM1 System On (GM1/GM2)

```
F0,7E,XN,09,01,F7
```

受信:○ / MDR再生:○

#### GM2 System On (GM2)

```
F0,7E,XN,09,03,F7
```

受信:○ / MDR再生:○

#### General MIDI System Off (GM1/GM2)

```
F0,7E,XN,09,02,F7
```

受信:○ / MDR再生:○

#### Scale/Octave Tuning (GM2)

```
F0,7E,XN,08,08,JJ,GG,MM,SS,…,F7
```

| バイト | 意味 |
|--------|------|
| XN | When N is received N=0-F, whichever is received. X=ignored |
| JJ | Channel/Option Byte1 (bit 0 = 15ch, bit 1 = 16ch, bit 2-6 = Reserved) |
| GG | Channel Byte2 (Bits0 to 6 = Channel 8 to 14) |
| MM | Channel Byte3 (Bits0 to 6 = Channel 1 to 7) |
| SS | 12byte tuning offset of 12 semitones from C to B (00-40-7F = -64...0...+63 [cent]) |

受信:○ / MDR再生:○

---

### System Exclusive Message (XG)

#### XGパラメータチェンジ

```
F0,43,1n,4C,hh,mm,ll,dd,…,F7
```

| バイト | 意味 |
|--------|------|
| 1n | Device Number n=always 0 (when transmit), n=0-F (when receive) |
| hh | Address High |
| mm | Address Mid |
| ll | Address Low |
| dd | Data |

受信:○ / MDR再生:○

#### XGバルクダンプ

```
F0,43,0n,4C,aa,bb,hh,mm,ll,dd,…,dd,cc,F7
```

| バイト | 意味 |
|--------|------|
| 0n | Device Number n=always 0 (when transmit), n=0-F (when receive) |
| aa | Byte Count MSB |
| bb | Byte Count LSB |
| hh | Address High |
| mm | Address Mid |
| ll | Address Low |
| dd | Data |
| cc | Checksum |

受信:○ / MDR再生:○

#### XGマスターチューニング

```
F0,43,1n,27,30,00,00,mm,pp,cc,F7
```

| バイト | 意味 |
|--------|------|
| 1n | Device Number n=always 0 (when transmit), n=0-F (when receive) |
| mm | Master Tune MSB |
| pp | Master Tune LSB |
| cc | Don't Care |

受信:○ / MDR再生:○

---

### System Exclusive Message (EL)

#### Controller

```
F0,43,70,70,40,cc,dd,F7
```

| バイト | 意味 |
|--------|------|
| cc | Controller Code |
| dd | Data |

- Controller = Volume Type: `00-7F`
- Controller = Switch Type: `00` (OFF), `7F` (ON)

受信:○ / 送信:○ / MDR再生:C / MDR録音:○

| cc | Controller | Type |
|----|-----------|------|
| 45 | Left Foot Switch | Switch |
| 46 | Right Foot Switch | Switch |
| 47 | Knee Lever | Switch |
| 51 | Slider 1 | Volume |
| 52 | Slider 2 | Volume |
| 53 | Slider 3 | Volume |
| 54 | Slider 4 | Volume |
| 55 | Slider 5 | Volume |
| 56 | Slider 6 | Volume |
| 57 | Slider 7 | Volume |
| 58 | Slider 8 | Volume |
| 59 | Slider 9 | Volume |
| 5A | Expression Pedal | Volume |
| 5B | External Pedal 1 | Volume |
| 5C | External Pedal 2 | Volume |

#### Tempo

```
F0,43,70,70,40,50,TL,TH,F7
```

| バイト | 意味 |
|--------|------|
| TL | Tempo LSB (bit 5-6 means bit 0-1, bit 0-4 = 0) |
| TH | Tempo MSB (bit 0-6 means bit 2-8) |

- Tempo Range: 40-280

受信:○ / 送信:○ / MDR再生:C / MDR録音:○

#### Panel Switch Event

```
F0,43,70,78,41,cc,dd,F7
```

| バイト | 意味 |
|--------|------|
| cc | Switch Code |
| dd | Data |

受信:○ / 送信:○ / MDR再生:C / MDR録音:○

#### Current Registration Data

```
F0,43,70,78,42,3C,dd,…,F7
```

| バイト | 意味 |
|--------|------|
| dd | Current Registration Data |

受信:○ / 送信:○ / MDR再生:C / MDR録音:○

#### MIDI Parameter

```
F0,43,70,78,44,hh,mm,ll,dd,…,F7
```

| バイト | 意味 |
|--------|------|
| hh | Address High |
| mm | Address Mid |
| ll | Address Low |
| dd | Data |

受信:○ / 送信:○ / MDR再生:C / MDR録音:○

#### EL ON

```
F0,43,70,70,73,F7
```

受信:○ / 送信:× / MDR再生:○ / MDR録音:×

#### Bar Signal

```
F0,43,70,70,78,SC,NC,F7
```

| バイト | 意味 |
|--------|------|
| SC | Beat Number（ELSシリーズは00固定） |
| NC | Bar Number（ELSシリーズは00固定） |

受信:× / 送信:○ / MDR再生:× / MDR録音:○

---

### System Exclusive Message (Others)

#### Internal Clock

```
F0,43,73,01,02,F7
```

受信:○ / MDR再生:○

#### External Clock

```
F0,43,73,01,03,F7
```

受信:○

#### リズムスタート

```
F0,43,6n,7A,F7
```

| バイト | 意味 |
|--------|------|
| 6n | Device Number n=always 0 (when transmit), n=0-F (when receive) |

受信:○ / MDR再生:○ / MDR録音:○

#### リズムストップ

```
F0,43,6n,7D,F7
```

| バイト | 意味 |
|--------|------|
| 6n | Device Number n=always 0 (when transmit), n=0-F (when receive) |

受信:○ / MDR再生:○ / MDR録音:○

---

## Switch Codes

フォーマット: `F0,43,70,78,41,cc,dd,F7`

- `cc` = Switch Code
- `dd` = Data (ELS-03)

### Selector

| cc | スイッチ | 受信値 | 受信 | 送信値 | 送信 | 備考 |
|----|---------|--------|------|--------|------|------|
| 0F | Registration Memory [1-16] | 00-0F | ○ | - | × | 00=Registration Memory 1、0F=Registration Memory 16 |

### Volume

| cc | スイッチ | 受信値 | 受信 | 送信値 | 送信 | 備考 |
|----|---------|--------|------|--------|------|------|
| 12 | Upper Voice 1 Volume | 00-7F | ○ | 00-7F | × | 00:MAX, 7F:MIN |
| 13 | Lower Voice 1 Volume | 00-7F | ○ | 00-7F | × | 00:MAX, 7F:MIN |
| 14 | Upper Voice 2 Volume | 00-7F | ○ | 00-7F | × | 00:MAX, 7F:MIN |
| 15 | Lower Voice 2 Volume | 00-7F | ○ | 00-7F | × | 00:MAX, 7F:MIN |
| 16 | Lead Voice 1 Volume | 00-7F | ○ | 00-7F | × | 00:MAX, 7F:MIN |
| 17 | Pedal Voice 1 Volume | 00-7F | ○ | 00-7F | × | 00:MAX, 7F:MIN |
| 18 | Pedal Voice 2 Volume | 00-7F | ○ | 00-7F | × | 00:MAX, 7F:MIN |
| 19 | Lead Voice 2 Volume | 00-7F | ○ | 00-7F | × | 00:MAX, 7F:MIN |
| 1A | Rhythm Volume (Percussion) | 00-7F | ○ | 00-7F | × | 00:MAX, 7F:MIN |
| 1B | Reverb Depth | 00-7F | ○ | 00-7F | × | 00:MAX, 7F:MIN |

### Organ Flute

| cc | スイッチ | 受信値 | 受信 | 送信値 | 送信 | 備考 |
|----|---------|--------|------|--------|------|------|
| 30 | Upper Organ Flute [U. ORGAN FLUTES] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |
| 31 | Lower Organ Flute [L. ORGAN FLUTES] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |

### Keyboard Assign

| cc | スイッチ | 受信値 | 受信 | 送信値 | 送信 | 備考 |
|----|---------|--------|------|--------|------|------|
| 36 | Lead Voice 1 Keyboard Assign [▼] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |
| 37 | Pedal Voice 1 Keyboard Assign [▲] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |
| 38 | Pedal Voice 2 Keyboard Assign [▲] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |

### Solo Mode

| cc | スイッチ | 受信値 | 受信 | 送信値 | 送信 | 備考 |
|----|---------|--------|------|--------|------|------|
| 39 | Lead Voice 2 Solo [SOLO(KNEE)] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |

### Brilliance

| cc | スイッチ | 受信値 | 受信 | 送信値 | 送信 | 備考 |
|----|---------|--------|------|--------|------|------|
| 42 | Upper Voice 1 Brilliance | 00-06 | ○ | 00-06 | × | 00:ブリリアント, 06:メロー |
| 43 | Lower Voice 1 Brilliance | 00-06 | ○ | 00-06 | × | 00:ブリリアント, 06:メロー |
| 44 | Upper Voice 2 Brilliance | 00-06 | ○ | 00-06 | × | 00:ブリリアント, 06:メロー |
| 45 | Lower Voice 2 Brilliance | 00-06 | ○ | 00-06 | × | 00:ブリリアント, 06:メロー |
| 46 | Lead Voice 1 Brilliance | 00-06 | ○ | 00-06 | × | 00:ブリリアント, 06:メロー |
| 47 | Pedal Voice 1 Brilliance | 00-06 | ○ | 00-06 | × | 00:ブリリアント, 06:メロー |
| 48 | Pedal Voice 2 Brilliance | 00-06 | ○ | 00-06 | × | 00:ブリリアント, 06:メロー |
| 49 | Lead Voice 2 Brilliance | 00-06 | ○ | 00-06 | × | 00:ブリリアント, 06:メロー |

### Sustain

| cc | スイッチ | 受信値 | 受信 | 送信値 | 送信 | 備考 |
|----|---------|--------|------|--------|------|------|
| 50 | Upper Sustain [UPPER(KNEE)] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |
| 51 | Lower Sustain [LOWER(KNEE)] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |
| 52 | Pedal Sustain [PEDAL] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |

### Slider Assign

| cc | スイッチ | 受信値 | 受信 | 送信値 | 送信 | 備考 |
|----|---------|--------|------|--------|------|------|
| 54 | Slider Assign | 00-04 | ○ | 00-04 | ○ | 00:Volume, 01:Brilliance, 02:Assignable, 03:Upper Organ Flute Footage, 04:Lower Organ Flute Footage |

### Keyboard Percussion

| cc | スイッチ | 受信値 | 受信 | 送信値 | 送信 | 備考 |
|----|---------|--------|------|--------|------|------|
| 5B | Keyboard Percussion [1] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |
| 5C | Keyboard Percussion [2] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |

### Disable

| cc | スイッチ | 受信値 | 受信 | 送信値 | 送信 | 備考 |
|----|---------|--------|------|--------|------|------|
| 5F | Disable [D.] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |

### Rotary Speaker

| cc | スイッチ | 受信値 | 受信 | 送信値 | 送信 | 備考 |
|----|---------|--------|------|--------|------|------|
| 60 | Rotary Speaker [ROTARY SP SPEED] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |

### Rhythm Sequence

| cc | スイッチ | 受信値 | 受信 | 送信値 | 送信 | 備考 |
|----|---------|--------|------|--------|------|------|
| 61 | Sequence 1 [SEQ.1] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |
| 62 | Sequence 2 [SEQ.2] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |
| 63 | Sequence 3 [SEQ.3] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |
| 64 | Sequence 4 [SEQ.4] | 00-01 | ○ | 00-01 | ○ | 00:OFF, 01:ON |

---

## MIDI Parameters

フォーマット: `F0,43,70,78,44,hh,mm,ll,dd,…,F7`

- `hh/mm/ll` = Address High/Mid/Low
- `dd` = Data

### 音群パラメータ

#### オーケストラ音群パラメーター

Address mm: 0-7 = UK1, UK2, LK1, LK2, Lead1, Lead2, PK1, PK2

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 | データ/備考 |
|----|----|----|------|-----------|------|--------|------|--------|------------|
| 10 | 00-07 | 00-0D | 5 | Voice Assign Number | ○ | 00-7F, 00-7F, 00-3F, 00-7F, 00-0F | ○ | 同左 | 音色番号 |
| 10 | 00-07 | 10 | 1 | Voice Select Number | ○ | 00-0D | ○ | 00-0D | 音色ボタン番号 |
| 10 | 00-07 | 11 | 1 | Volume | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 10 | 00-07 | 12 | 1 | Reverb Send Level | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 10 | 00-07 | 13 | 1 | Brilliance | ○ | 00-7F | ○ | 00-7F | Brilliant〜Mellow |
| 10 | 00-07 | 14 | 1 | Feet | ○ | 00=Preset, 01=16Feet, 02=8Feet, 03=4Feet, 04=2Feet, 05-7F | ○ | 00-04 | 04(2Feet)はPK1/2のみ有効 |
| 10 | 00-07 | 15 | 1 | Pan | ○ | 00-7F | ○ | 08-78 | Left〜Center(40)〜Right |
| 10 | 00-07 | 16 | 1 | Touch Tone Initial Touch | ○ | 00-7F | ○ | 00-7F | No Effect〜Wide |
| 10 | 00-07 | 17 | 1 | Touch Tone After Touch | ○ | 00-7F | ○ | 00-7F | No Effect〜Wide |
| 10 | 00-07 | 18 | 1 | Pitch After Touch | ○ | 00-7F | ○ | 32-4E | -14〜0(40)〜+14 |
| 10 | 00-07 | 19 | 1 | User Vibrato | ○ | 00=Preset, 01-7F=User | ○ | 00-01 | |
| 10 | 00-07 | 1A | 1 | Vibrato Delay | ○ | 00-7F | ○ | 02-1A | Short〜Long |
| 10 | 00-07 | 1B | 1 | Vibrato Depth | ○ | 00-7F | ○ | 00-54 | Min〜Max |
| 10 | 00-07 | 1C | 1 | Vibrato Speed | ○ | 00-7F | ○ | 3C-6C | Slow〜Fast |
| 10 | 00-05 | 1D | 1 | Pitch Horizontal Touch | ○ | 00-7F | ○ | 00-7F | No Effect〜Wide |
| 10 | 00-07 | 1E | 1 | Touch Vibrato On/Off | ○ | 00=Off, 01-7F=On | ○ | 00=Off, 7F=On | |
| 10 | 04-07 | 1F | 1 | Keyboard Assign / Solo | ○ | 00=Off, 01-7F=On | × | - | |
| 10 | 04-05 | 20 | 1 | Slide | ○ | 00=Off, 01=On, 02=Knee Control | ○ | 00-02 | |
| 10 | 04-05 | 21 | 1 | Slide Time | ○ | 00-7F | ○ | 02-7F | Fast〜Slow |
| 10 | 00-07 | 22 | 1 | Tune | ○ | 00-7F | ○ | 00-7F | -64〜0(40)〜+63 |
| 10 | 00-01, 04-07 | 23 | 1 | 2nd Expression Pitch Bend | ○ | 00=Off, 01-7F=On | × | - | mm 00-01はOff/On、04-07はOn |
| 10 | 00-05 | 24 | 1 | Foot Switch Glide Control | ○ | 00=Off, 01-7F=On | × | - | |
| 10 | 00-07 | 25 | 1 | Transpose | ○ | 3A-46 | ○ | 3A-46 | -6〜0(40)〜+6 |
| 10 | 06-07 | 28 | 1 | Poly/Mono | ○ | 00=Mono, 01-7F=Poly | ○ | 00-01 | |
| 10 | 05 | 29 | 1 | Priority (Last/Top) | ○ | 00=Top, 01-7F=Last | ○ | 00-01 | |
| 10 | 00-07 | 2A | 1 | Part On/Off | ○ | 00=On, 01-7F=Off(Mute) | ○ | 00-01 | |
| 10 | 00-07 | 40 | 3 | Effect 1 Type | ○ | 00, 00-7F(MSB), 00-7F(LSB) | ○ | 同左 | |
| 10 | 00-07 | 41-50 | 2 each | Effect 1 Parameter 1-16 MSB/LSB | ○ | 0000-7F7F | ○ | 0000-7F7F | |
| 10 | 00-07 | 51 | 3 | Effect 2 Type | ○ | 00, 00-7F(MSB), 00-7F(LSB) | ○ | 同左 | |
| 10 | 00-07 | 52-61 | 2 each | Effect 2 Parameter 1-16 MSB/LSB | ○ | 0000-7F7F | ○ | 0000-7F7F | |
| 10 | 00-07 | 63 | 1 | Sustain Length | ○ | 7F=Hold, 00-7E=Short〜Long | ○ | 7F=Hold, 15-3D | |
| 10 | 00-07 | 64 | 1 | Articulation Foot Switch Left | ○ | 00=Off, 01=Art.1, 02=Art.2, 03=Art.3 | ○ | 00-03 | |
| 10 | 00-07 | 65 | 1 | Articulation Auto On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 | |
| 10 | 04,06,07 | 66 | 1 | Assigned Keyboard | ○ | 00=Upper, 01/03-7F=Lower, 02=Pedal | ○ | 00-02 | |
| 10 | 00-07 | 67 | 1 | After Fine Tune | ○ | 00-7F | ○ | 00-7F | -64〜0(40)〜+63 |
| 10 | 00-07 | 68 | 1 | After Cutoff Frequency | ○ | 00-7F | ○ | 00-7F | -64〜0(40)〜+63 |
| 10 | 00-07 | 69 | 1 | After Resonance | ○ | 00-7F | ○ | 00-7F | -64〜0(40)〜+63 |
| 10 | 00-07 | 6A | 1 | After PMOD | ○ | 00-7F | ○ | 00-7F | 0〜127 |
| 10 | 00-07 | 6B | 1 | After FMOD | ○ | 00-7F | ○ | 00-7F | 0〜127 |
| 10 | 00-07 | 6C | 1 | After AMOD | ○ | 00-7F | ○ | 00-7F | 0〜127 |
| 10 | 00-07 | 6D | 1 | After LFO Speed | ○ | 00-7F | ○ | 00-7F | 0〜127 |
| 10 | 00-07 | 6E | 1 | After Template | ○ | 00=Off, 01-05=1〜5 | ○ | 00-05 | |
| 10 | 00-07 | 6F | 1 | Polyphonic After Touch | ○ | 00/02-7F=Off, 01=On | ○ | 00-01 | |

#### オルガンフルートボイスパラメーター

Address mm: 0-1 = Upper, Lower

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 | データ/備考 |
|----|----|----|------|-----------|------|--------|------|--------|------------|
| 11 | 00-01 | 00 | 1 | Footage 16 Feet | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 11 | 00-01 | 01 | 1 | Footage 8 Feet | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 11 | 00-01 | 02 | 1 | Footage 5-1/3 Feet | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 11 | 00-01 | 03 | 1 | Footage 4 Feet | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 11 | 00-01 | 04 | 1 | Footage 2-2/3 Feet | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 11 | 00-01 | 05 | 1 | Footage 2 Feet | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 11 | 00-01 | 06 | 1 | Footage 1-3/5 Feet | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 11 | 00-01 | 07 | 1 | Footage 1-1/3 Feet | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 11 | 00-01 | 08 | 1 | Footage 1 Feet | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 11 | 00-01 | 09 | 1 | Attack Response | ○ | 00-7F | ○ | 00-7F | Fast〜Slow |
| 11 | 00-01 | 0A | 1 | Attack 4 Feet | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 11 | 00-01 | 0B | 1 | Attack 2-2/3 Feet | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 11 | 00-01 | 0C | 1 | Attack 2 Feet | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 11 | 00-01 | 0D | 1 | Attack Length | ○ | 00-7F | ○ | 00-7F | Short〜Long |
| 11 | 00-01 | 10 | 1 | Organ Flute On/Off | ○ | 00=Off, 01-7F=On | × | - | |
| 11 | 00-01 | 11 | 1 | Volume | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 11 | 00-01 | 12 | 1 | Reverb Send Level | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 11 | 00-01 | 13 | 1 | Organ Flute Type | ○ | 00=Sine, 01/03-7F=Vintage, 02=Euro | ○ | 00-02 | |
| 11 | 00-01 | 19 | 1 | Vibrato On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 | |
| 11 | 00-01 | 1B | 1 | Vibrato Depth | ○ | 00-02 | ○ | 00-02 | Min〜Max |
| 11 | 00-01 | 1C | 1 | Vibrato Speed | ○ | 00-3F | ○ | 00-3F | Slow〜Fast |
| 11 | 00-01 | 40 | 3 | Effect Type | ○ | 00, 00-7F(MSB), 00-7F(LSB) | ○ | 同左 | |
| 11 | 00-01 | 41-50 | 2 each | Effect Parameter 1-16 MSB/LSB | ○ | 0000-7F7F | ○ | 0000-7F7F | |
| 11 | 00-01 | 63 | 1 | Sustain Length | ○ | 7F=Hold, 00-7E=Short〜Long | ○ | 7F=Hold, 15-37 | |
| 11 | 00 | 64 | 1 | Organ Group | ○ | 00=Organ Flutes, 01-7F=VCM Organ | ○ | 00-01 | 上下鍵盤共通 |
| 11 | 00 | 65 | 1 | VCM Organ Type | ○ | 01=Standard, 02=Live, 03=Percussive | ○ | 01-03 | 上下鍵盤共通 |
| 11 | 00 | 66 | 1 | Percussion On/Off | ○ | 00/02-7F=Off, 01=On | ○ | 00-01 | 上鍵盤のみ |
| 11 | 00 | 67 | 1 | Percussion Normal/Soft | ○ | 00/02-7F=Normal, 01=Soft | ○ | 00-01 | 上鍵盤のみ |
| 11 | 00 | 68 | 1 | Percussion Slow/Fast | ○ | 00/02-7F=Slow, 01=Fast | ○ | 00-01 | 上鍵盤のみ |
| 11 | 00 | 69 | 1 | Percussion 2nd/3rd | ○ | 00=2nd, 01-7F=3rd | ○ | 00-01 | 上鍵盤のみ |
| 11 | 00 | 6A | 1 | Expression Type | ○ | 00/02-7F=Drive+Volume, 01=Volume | ○ | 00-01 | 上下鍵盤共通 |
| 11 | 00 | 6B | 1 | Leak Level | ○ | 00-7F | ○ | 00-7F | 0(Min)〜127(Max)、上下鍵盤共通 |
| 11 | 00 | 6C | 1 | Key Click Level | ○ | 00-7F | ○ | 00-7F | 0(Min)〜127(Max)、上下鍵盤共通 |
| 11 | 00 | 6D | 1 | Pre Drive | ○ | 00-7F | ○ | 00-7F | 0(Min)〜127(Max)、上下鍵盤共通 |
| 11 | 00 | 6E | 1 | Vibrato/Chorus Type | ○ | 00=V1, 01=C1, 02=V2, 03=C2, 04=V3, 05-7F=C3 | ○ | 00-05 | 上下鍵盤共通 |
| 11 | 00 | 6F | 1 | VCM Rotary Speaker Type | ○ | 00=Classic, 01=Overdrive, 02=Studio | ○ | 00-02 | 上下鍵盤共通 |
| 11 | 00 | 70 | 1 | VCM Rotary Speaker Drive | ○ | 00-7F | ○ | 00-7F | 0(Min)〜127(Max)、上下鍵盤共通 |
| 11 | 00 | 71 | 1 | VCM Rotary Speaker Tone | ○ | 00-7F | ○ | 00-7F | 0(Min)〜127(Max)、上下鍵盤共通 |
| 11 | 00 | 72 | 1 | VCM Rotary Speaker Level (Horn) | ○ | 00-7F | ○ | 00-7F | 0(Min)〜127(Max)、上下鍵盤共通 |
| 11 | 00 | 73 | 1 | VCM Rotary Speaker Level (Rotor) | ○ | 00-7F | ○ | 00-7F | 0(Min)〜127(Max)、上下鍵盤共通 |

---

### 鍵盤パラメーター

#### サステインパラメーター

Address mm: 0-2 = UK, LK, PK

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 備考 |
|----|----|----|------|-----------|------|--------|------|------|
| 12 | 00-02 | 00 | 1 | Sustain On/Off | ○ | 00=Off, 01-7F=On | × | |
| 12 | 00-02 | 01 | 1 | Sustain Length | ○ | 7F=Hold, 00-7E=Short〜Long | × | |

#### キーボードパーカッションパラメーター

Address mm: 1-2 = KBP1, KBP2

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 | 備考 |
|----|----|----|------|-----------|------|--------|------|--------|------|
| 12 | 01-02 | 10 | 1 | Keyboard Percussion On/Off [K.B.P.(ON/OFF)] | ○ | 00=Off, 01-7F=On | × | 00-01 | |
| 12 | 01-02 | 11 | 1 | Keyboard Percussion Menu [K.B.P. MENU] | ○ | 00=Preset, 01-28=User1〜40 | ○ | 00-28 | |

---

### リズム

#### リズムパラメーター

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 | 備考 |
|----|----|----|------|-----------|------|--------|------|--------|------|
| 13 | 00 | 00-0B | 3 | Rhythm Assign Number | ○ | 00-7F, 00-7F, 00-0F | ○ | 同左 | リズム番号 |
| 13 | 00 | 10 | 1 | Rhythm Select Number | ○ | 00-0B | ○ | 00-0B | リズムボタン番号 |
| 13 | 00 | 11 | 1 | Percussion Volume | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 13 | 00 | 12 | 1 | Percussion Reverb Send Level | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 13 | 00 | 13 | 1 | 2nd Expression Tempo Control On/Off | ○ | 00=Off, 01-7F=On | × | - | |
| 13 | 00 | 14 | 1 | Left Foot Sw Rhythm Section | ○ | 00=Intro1, 01=Intro2, 02=Intro3, 08=MainA, 09=MainB, 0A=MainC, 0B=MainD, 18=Break, 20=Ending1, 21=Ending2, 22=Ending3, 7E=Stop, 7F=Off | ○ | 同左 | |
| 13 | 00 | 1F | 1 | Right Foot Sw Rhythm Section | ○ | * | ○ | * | Left Foot Sw Rhythm Sectionと同様 |
| 13 | 00 | 20 | 1 | External Pedal 1 Rhythm Section | ○ | * | ○ | * | Left Foot Sw Rhythm Sectionと同様 |
| 13 | 00 | 21 | 1 | External Pedal 2 Rhythm Section | ○ | * | ○ | * | Left Foot Sw Rhythm Sectionと同様 |
| 13 | 00 | 15 | 1 | Add Drum Part On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 | |
| 13 | 00 | 16 | 1 | Main Drum Part On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 | |
| 13 | 00 | 17 | 1 | Chord 1 Part On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 | |
| 13 | 00 | 18 | 1 | Chord 2 Part On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 | |
| 13 | 00 | 19 | 1 | Pad Part On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 | |
| 13 | 00 | 1A | 1 | Phrase 1 Part On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 | |
| 13 | 00 | 1B | 1 | Phrase 2 Part On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 | |
| 13 | 00 | 1C | 1 | Auto Fill On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 | |

#### リズムシーケンス

Address ll: 0-3 = Sequence 1〜4

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 備考 |
|----|----|----|------|-----------|------|--------|------|------|
| 13 | 01 | 00-03 | 1 | Rhythm Sequence 1〜4 On/Off | ○ | 00=Off, 01-7F=On | × | |

#### アカンパニメントパラメーター

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 | 備考 |
|----|----|----|------|-----------|------|--------|------|--------|------|
| 13 | 02 | 11 | 1 | Accompaniment Volume | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 13 | 02 | 12 | 1 | Accompaniment Reverb Send Level | ○ | 00-7F | ○ | 00-7F | Min〜Max |

#### A.B.C.設定パラメーター

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 | 備考 |
|----|----|----|------|-----------|------|--------|------|--------|------|
| 13 | 03 | 00 | 1 | オートベースコード モード [A.B.C. MODE] | ○ | 00/04-7F=Off, 01=Single Finger, 02=Fingered, 03=Custom A.B.C. | ○ | 00-03 | |
| 13 | 03 | 01 | 1 | Lower Keyboard Memory On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 | |
| 13 | 03 | 02 | 1 | Pedal Keyboard Memory On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 | |

#### M.O.C.設定パラメーター

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 | 備考 |
|----|----|----|------|-----------|------|--------|------|--------|------|
| 13 | 04 | 00 | 1 | M.O.C. Mode | ○ | 00/04-7F=Off, 01=Mode1, 02=Mode2, 03=Mode3 | ○ | 00-03 | |
| 13 | 04 | 01 | 1 | M.O.C. On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 | |

#### セクションパラメーター

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 |
|----|----|----|------|-----------|------|--------|------|--------|
| 13 | 05 | 00 | 1 | Intro 1 On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 |
| 13 | 05 | 01 | 1 | Intro 2 On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 |
| 13 | 05 | 02 | 1 | Intro 3 On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 |
| 13 | 05 | 08 | 1 | Main A On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 |
| 13 | 05 | 09 | 1 | Main B On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 |
| 13 | 05 | 0A | 1 | Main C On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 |
| 13 | 05 | 0B | 1 | Main D On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 |
| 13 | 05 | 18 | 1 | Break On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 |
| 13 | 05 | 20 | 1 | Ending 1 On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 |
| 13 | 05 | 21 | 1 | Ending 2 On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 |
| 13 | 05 | 22 | 1 | Ending 3 On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 |

#### K.B.P.パラメーター

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 | 備考 |
|----|----|----|------|-----------|------|--------|------|--------|------|
| 13 | 10 | 11 | 1 | KBP Volume | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 13 | 10 | 12 | 1 | KBP Reverb Send Level | ○ | 00-7F | ○ | 00-7F | Min〜Max |

---

### 全体

#### 全体パラメータ

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 | 備考 |
|----|----|----|------|-----------|------|--------|------|--------|------|
| 14 | 00 | 00 | 1 | Disable On/Off | ○ | 00=Off, 01-7F=On | × | - | |
| 14 | 00 | 01 | 1 | Organ Flute Attack Mode | ○ | 00=Each, 01-7F=First | ○ | 00-01 | |
| 14 | 00 | 02 | 1 | Pitch Control Transpose | ○ | 3A-46 | ○ | 3A-46 | -6〜0(40)〜+6 |
| 14 | 00 | 03 | 1 | 2nd Expression Range | ○ | 01-0C | × | - | Narrow〜Wide |
| 14 | 00 | 04 | 1 | Left Foot Switch Mode | ○ | 00/04-7F=Off, 01=Rhythm, 02=Glide, 03=Rotary Speaker | × | - | |
| 14 | 00 | 05 | 1 | Master Tune | ○ | 00-7F | ○ | 00-7F | 427.2Hz〜440.0Hz(40)〜452.6Hz |
| 14 | 00 | 06 | 1 | Glide Time | ○ | 00-7F | ○ | 04-1C | Fast〜Slow |
| 14 | 00 | 08 | 1 | MIDI Control Expression Internal/External | ○ | 00/02-7F=Internal, 01=External | × | 00-01 | パネルの設定がAUTOの時のみ有効 |
| 14 | 00 | 09 | 1 | MIDI Control Lead 1 Internal/External | ○ | 00=Internal, 01-7F=External | × | - | |
| 14 | 00 | 0A | 3 | Registration Menu | ○ | 00-04(Sw), 00-2F(Page/Category), 00-0B(Position) | ○ | 同左 | bit0-3:Page1〜16, bit4-6:Category(0=Vol.1/ELS-01, 1=Vol.2/ELS-02, 2=Vol.3/ELS-03) |
| 14 | 00 | 0B | 1 | Disable Mode | ○ | 00=Normal, 01=Tempo | ○ | 00-01 | |
| 14 | 00 | 0C | 1 | IAC On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 | |

#### 全体エフェクトパラメータ：リバーブ

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 | 備考 |
|----|----|----|------|-----------|------|--------|------|--------|------|
| 14 | 01 | 00 | 1 | Reverb Depth | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 14 | 01 | 01 | 1 | Reverb Time (Panel) | ○ | 00-7F | ○ | 00-64 | Short〜Long |
| 14 | 01 | 02 | 3 | Reverb Type (Panel) | ○ | 00, 00-7F(MSB), 00-7F(LSB) | ○ | 同左 | |

#### 全体エフェクトパラメータ：リバーブリズム

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 | 備考 |
|----|----|----|------|-----------|------|--------|------|--------|------|
| 14 | 02 | 01 | 1 | Reverb Time (Rhythm) | ○ | 00-7F | ○ | 00-64 | Short〜Long |
| 14 | 02 | 02 | 3 | Reverb Type (Rhythm) | ○ | 00, 00-7F(MSB), 00-7F(LSB) | ○ | 同左 | |

#### 全体エフェクトパラメータ：ロータリースピーカー

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 | 備考 |
|----|----|----|------|-----------|------|--------|------|--------|------|
| 14 | 03 | 00 | 1 | Rotary Speaker Speed On/Off | ○ | 00=Off, 01-7F=On | × | - | |
| 14 | 03 | 01 | 1 | Rotary Speaker Speed Control Mode | ○ | 00=Stop, 01-7F=Slow | ○ | 00-01 | |
| 14 | 03 | 02 | 2 | Rotary Speaker Speed Control Speed | ○ | 0000-7F7F | ○ | 0040-007F | 2.69〜39.7Hz |
| 14 | 03 | 03 | 2 | VCM Rotary Speaker Speed (Horn) | ○ | 0000-7F7F | ○ | 0001-007F | 3.49〜13.63 [Hz] |
| 14 | 03 | 04 | 2 | VCM Rotary Speaker Speed (Rotor) | ○ | 0000-7F7F | ○ | 0001-007F | 3.16〜12.28 [Hz] |

#### Live Expression Control

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 |
|----|----|----|------|-----------|------|--------|------|--------|
| 14 | 06 | 00 | 1 | Expression Pedal Function | ○ | 00-7F | ○ | 00-7F |
| 14 | 06 | 01 | 1 | 2nd Expression Pedal Function | ○ | 00-7F | ○ | 00-7F |
| 14 | 06 | 02 | 1 | Right Foot Sw Function | ○ | 00-7F | ○ | 00-7F |
| 14 | 06 | 03 | 1 | Left Foot Sw Function | ○ | 00-7F | ○ | 00-7F |
| 14 | 06 | 04 | 1 | External Pedal 1 Function | ○ | 00-7F | ○ | 00-7F |
| 14 | 06 | 05 | 1 | External Pedal 2 Function | ○ | 00-7F | ○ | 00-7F |
| 14 | 06 | 06 | 1 | Slider 1 Function | ○ | 00-7F | ○ | 00-7F |
| 14 | 06 | 07 | 1 | Slider 2 Function | ○ | 00-7F | ○ | 00-7F |
| 14 | 06 | 08 | 1 | Slider 3 Function | ○ | 00-7F | ○ | 00-7F |
| 14 | 06 | 09 | 1 | Slider 4 Function | ○ | 00-7F | ○ | 00-7F |
| 14 | 06 | 0A | 1 | Slider 5 Function | ○ | 00-7F | ○ | 00-7F |
| 14 | 06 | 0B | 1 | Slider 6 Function | ○ | 00-7F | ○ | 00-7F |
| 14 | 06 | 0C | 1 | Slider 7 Function | ○ | 00-7F | ○ | 00-7F |
| 14 | 06 | 0D | 1 | Slider 8 Function | ○ | 00-7F | ○ | 00-7F |
| 14 | 06 | 0E | 1 | Slider 9 Function | ○ | 00-7F | ○ | 00-7F |
| 14 | 06 | 20 | 4 | Expression Pedal Part | ○ | 00000000-7F7F7F7F | ○ | 00000000-7F7F7F7F |
| 14 | 06 | 21 | 4 | 2nd Expression Pedal Part | ○ | 00000000-7F7F7F7F | ○ | 00000000-7F7F7F7F |
| 14 | 06 | 22 | 4 | Right Foot Sw Part | ○ | 00000000-7F7F7F7F | ○ | 00000000-7F7F7F7F |
| 14 | 06 | 23 | 4 | Left Foot Sw Part | ○ | 00000000-7F7F7F7F | ○ | 00000000-7F7F7F7F |
| 14 | 06 | 24 | 4 | External Pedal 1 Part | ○ | 00000000-7F7F7F7F | ○ | 00000000-7F7F7F7F |
| 14 | 06 | 25 | 4 | External Pedal 2 Part | ○ | 00000000-7F7F7F7F | ○ | 00000000-7F7F7F7F |
| 14 | 06 | 26 | 4 | Slider 1 Part | ○ | 00000000-7F7F7F7F | ○ | 00000000-7F7F7F7F |
| 14 | 06 | 27 | 4 | Slider 2 Part | ○ | 00000000-7F7F7F7F | ○ | 00000000-7F7F7F7F |
| 14 | 06 | 28 | 4 | Slider 3 Part | ○ | 00000000-7F7F7F7F | ○ | 00000000-7F7F7F7F |
| 14 | 06 | 29 | 4 | Slider 4 Part | ○ | 00000000-7F7F7F7F | ○ | 00000000-7F7F7F7F |
| 14 | 06 | 2A | 4 | Slider 5 Part | ○ | 00000000-7F7F7F7F | ○ | 00000000-7F7F7F7F |
| 14 | 06 | 2B | 4 | Slider 6 Part | ○ | 00000000-7F7F7F7F | ○ | 00000000-7F7F7F7F |
| 14 | 06 | 2C | 4 | Slider 7 Part | ○ | 00000000-7F7F7F7F | ○ | 00000000-7F7F7F7F |
| 14 | 06 | 2D | 4 | Slider 8 Part | ○ | 00000000-7F7F7F7F | ○ | 00000000-7F7F7F7F |
| 14 | 06 | 2E | 4 | Slider 9 Part | ○ | 00000000-7F7F7F7F | ○ | 00000000-7F7F7F7F |
| 14 | 06 | 40 | 4 | Live Control Balance | ○ | 00000000-7F7F7F7F | ○ | 00000000-7F7F7F7F |
| 14 | 06 | 41 | 1 | Live Control Pitch Bend Range | ○ | 01-0C | ○ | 01-0C | Narrow〜Wide |
| 14 | 06 | 42 | 1 | Live Control Tempo Range | ○ | 01-0C | ○ | 01-0C | Narrow〜Wide |
| 14 | 06 | 50 | 1 | Slider Mode | ○ | 00=Jump, 01=Catch | ○ | 00-01 | |
| 14 | 06 | 51 | 1 | Live Control Behavior | ○ | 00=Continue, 01=Reset | ○ | 00-01 | |
| 14 | 06 | 7F | 1 | Reset Value | ○ | 00 | ○ | 00 | |

#### Master EQ/Comp

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 | 備考 |
|----|----|----|------|-----------|------|--------|------|--------|------|
| 14 | 07 | 00 | 1 | Master EQ Type | ○ | 00=Flat, 01=Mellow, 02=Bright, 03=Loudness, 04=Powerful, 05=Compatible | ○ | 00-05 | |
| 14 | 07 | 01 | 1 | Master EQ Frequency (Band 1) | ○ | 00-7F | ○ | 04-28 | 32Hz〜2.0kHz |
| 14 | 07 | 02 | 1 | Master EQ Frequency (Band 2) | ○ | 00-7F | ○ | 0E-36 | 100Hz〜10kHz |
| 14 | 07 | 03 | 1 | Master EQ Frequency (Band 3) | ○ | 00-7F | ○ | 0E-36 | 100Hz〜10kHz |
| 14 | 07 | 04 | 1 | Master EQ Frequency (Band 4) | ○ | 00-7F | ○ | 0E-36 | 100Hz〜10kHz |
| 14 | 07 | 05 | 1 | Master EQ Frequency (Band 5) | ○ | 00-7F | ○ | 0E-36 | 100Hz〜10kHz |
| 14 | 07 | 06 | 1 | Master EQ Frequency (Band 6) | ○ | 00-7F | ○ | 0E-36 | 100Hz〜10kHz |
| 14 | 07 | 07 | 1 | Master EQ Frequency (Band 7) | ○ | 00-7F | ○ | 0E-36 | 100Hz〜10kHz |
| 14 | 07 | 08 | 1 | Master EQ Frequency (Band 8) | ○ | 00-7F | ○ | 1C-3A | 500Hz〜16kHz |
| 14 | 07 | 09 | 1 | Master EQ Gain (Band 1) | ○ | 00-7F | ○ | 34-4C | -12〜0(40)〜+12dB |
| 14 | 07 | 0A | 1 | Master EQ Gain (Band 2) | ○ | 00-7F | ○ | 34-4C | -12〜0(40)〜+12dB |
| 14 | 07 | 0B | 1 | Master EQ Gain (Band 3) | ○ | 00-7F | ○ | 34-4C | -12〜0(40)〜+12dB |
| 14 | 07 | 0C | 1 | Master EQ Gain (Band 4) | ○ | 00-7F | ○ | 34-4C | -12〜0(40)〜+12dB |
| 14 | 07 | 0D | 1 | Master EQ Gain (Band 5) | ○ | 00-7F | ○ | 34-4C | -12〜0(40)〜+12dB |
| 14 | 07 | 0E | 1 | Master EQ Gain (Band 6) | ○ | 00-7F | ○ | 34-4C | -12〜0(40)〜+12dB |
| 14 | 07 | 0F | 1 | Master EQ Gain (Band 7) | ○ | 00-7F | ○ | 34-4C | -12〜0(40)〜+12dB |
| 14 | 07 | 10 | 1 | Master EQ Gain (Band 8) | ○ | 00-7F | ○ | 34-4C | -12〜0(40)〜+12dB |
| 14 | 07 | 11 | 1 | Master EQ Q (Band 1) | ○ | 00-7F | ○ | 01-78 | 0.1〜12.0 |
| 14 | 07 | 12 | 1 | Master EQ Q (Band 2) | ○ | 00-7F | ○ | 01-78 | 0.1〜12.0 |
| 14 | 07 | 13 | 1 | Master EQ Q (Band 3) | ○ | 00-7F | ○ | 01-78 | 0.1〜12.0 |
| 14 | 07 | 14 | 1 | Master EQ Q (Band 4) | ○ | 00-7F | ○ | 01-78 | 0.1〜12.0 |
| 14 | 07 | 15 | 1 | Master EQ Q (Band 5) | ○ | 00-7F | ○ | 01-78 | 0.1〜12.0 |
| 14 | 07 | 16 | 1 | Master EQ Q (Band 6) | ○ | 00-7F | ○ | 01-78 | 0.1〜12.0 |
| 14 | 07 | 17 | 1 | Master EQ Q (Band 7) | ○ | 00-7F | ○ | 01-78 | 0.1〜12.0 |
| 14 | 07 | 18 | 1 | Master EQ Q (Band 8) | ○ | 00-7F | ○ | 01-78 | 0.1〜12.0 |
| 14 | 07 | 19 | 1 | Master EQ Bass Shape | ○ | 00/02-7F=Shelving, 01=Peaking | ○ | 00-01 | |
| 14 | 07 | 1A | 1 | Master EQ Treble Shape | ○ | 00/02-7F=Shelving, 01=Peaking | ○ | 00-01 | |
| 14 | 07 | 40 | 1 | Master Comp On/Off | ○ | 00=Off, 01-7F=On | ○ | 00-01 | |
| 14 | 07 | 41 | 1 | Master Comp Type | ○ | 00=Limiter, 01=Bright, 02=Powerful, 03=Smooth, 04=Punchy | ○ | 00-04 | |
| 14 | 07 | 42 | 1 | Master Comp Threshold | ○ | 00-78 | ○ | 00-78 | -60dB〜±0dB |
| 14 | 07 | 43 | 1 | Master Comp Knee | ○ | 00-28 | ○ | 00-28 | -20dB〜±0dB |
| 14 | 07 | 44 | 1 | Master Comp Ratio | ○ | 00-0F | ○ | 00-0F | 1〜20, ∞ |
| 14 | 07 | 45 | 1 | Master Comp Release | ○ | 00-64 | ○ | 00-64 | 5ms〜500ms |
| 14 | 07 | 46 | 1 | Master Comp Makeup Gain | ○ | 00-50 | ○ | 00-50 | -20dB〜+20dB |

#### Mic

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 | 備考 |
|----|----|----|------|-----------|------|--------|------|--------|------|
| 14 | 08 | 00 | 1 | Vocal Volume | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 14 | 08 | 01 | 1 | Vocal Reverb Send Level | ○ | 00-7F | ○ | 00-7F | Min〜Max |
| 14 | 08 | 02 | 1 | Pitch Detect Voice Range | ○ | 00=Bass, 01=Alto/Tenor, 02=Soprano, 03=All Range | ○ | 00-03 | |
| 14 | 08 | 03 | 1 | Pitch Detect Harmony Response | ○ | 00=Slow, 01=Medium Slow, 02=Medium, 03=Medium Fast, 04=Fast | ○ | 00-04 | |

#### Vocal Harmony

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 | 備考 |
|----|----|----|------|-----------|------|--------|------|--------|------|
| 14 | 09 | 00 | 2 | Vocal Harmony Type | ○ | 0C=Vocal Harmony, 0D=Synth Vocoder, 00-0B/0E-7F=Thru; 00-7F=Type | ○ | 0C/0D/40=Thru; 00-7F=Type | |
| 14 | 09 | 01 | 1 | Vocal Harmony Switch | ○ | 00/02-7F=Off, 01=On | ○ | 00-01 | |
| 14 | 09 | 02 | 1 | Vocal Harmony Lead Volume | ○ | 00-7F | ○ | 00-7F | 0〜127 |
| 14 | 09 | 03 | 1 | Vocal Harmony Harmony Volume | ○ | 00-7F | ○ | 00-7F | 0〜127 |
| 14 | 09 | 04 | 1 | Vocoder Keyboard Part | ○ | 00=Upper, 01=Lower, 02=Pedal | ○ | 00-02 | |
| 14 | 09 | 05 | 1 | Vocal Effect Switch | ○ | 00/02-7F=Off, 01=On | ○ | 00-01 | |
| 14 | 09 | 06 | 1 | Vocal Effect To Lead | ○ | 00-7F | ○ | 00-7F | 0〜127 |
| 14 | 09 | 07 | 1 | Vocal Effect To Harmony | ○ | 00-7F | ○ | 00-7F | 0〜127 |

---

### イベント

| hh | mm | ll | Size | Parameter | 受信 | 受信値 | 送信 | 送信値 | 備考 |
|----|----|----|------|-----------|------|--------|------|--------|------|
| 7E | 00 | 00 | 1 | Registration Bank | ○ | 00-04 | ○ | 00-04 | Bank A〜E |

---

## MIDIインプリメンテーションチャート

Date: 30-Nov-2025 / Version: 1.00

### EL Mode — YAMAHA [Electone-EL Mode]

対象: ELS-03XF, ELS-03XR, ELS-03X, ELS-03G

| Function | Transmitted | Recognized | Remarks |
|----------|-------------|------------|---------|
| Basic Channel — Default | 1,2,3,16 (*1) | 1-3,5-16 (*2) | |
| Basic Channel — Changed | 1-16 | 4 | |
| Mode — Default | Mode 3 | Mode 3 | |
| Mode — Messages | × | × | |
| Mode — Altered | *************** | × | |
| Note Number | 36-96 (*3) | 0-127 (*4) | |
| Note Number :True voice | *************** | | |
| Velocity — Note on | ○ 9nH,v=1-127 | ○ 9nH,v=1-127 | |
| Velocity — Note off | × 9nH,v=0 | × 9nH,v=0 or 8nH | |
| After Touch — Key's | ○ (*12) | ○ | |
| After Touch — Ch's | ○ | ○ | |
| Pitch Bend | ○ (*5) | ○ | |
| Control Change 0,32 | × | ○ (*6) | Bank Select |
| Control Change 1,5,7,10 | × | ○ (*6) | |
| Control Change 4 | ○ (*7) | ○ (*7) | 2nd Expression |
| Control Change 6,38 | × | ○ (*6) | Data Entry |
| Control Change 11 | ○ (*7) | ○ (*6,*7) | Expression |
| Control Change 16 | ○ (*8,*12) | ○ (*8,*12) | VA After Touch |
| Control Change 96,97 | × | ○ (*6) | Data Entry SW |
| Control Change 64-67 | × | ○ (*6) | |
| Control Change 71-78 | × | ○ (*6) | Sound Controller |
| Control Change 84,91,93,94 | × | ○ (*6) | |
| Control Change 98-99,100-101 | × | ○ (*6) | NRPN, RPN |
| Program Change | ○ (*10) | ○ (*11) | |
| Program Change :True number | *************** | | |
| System Exclusive | ○ | ○ | |
| System Common :Song Position | × | × | |
| System Common :Song Select | × | × | |
| System Common :Tune | × | × | |
| System Real Time :Clock | ○ | ○ (*9) | |
| System Real Time :Commands | ○ | ○ | (FAH, FCH) |
| Aux Messages :All Sound Off | × | ○ (120) (*6) | |
| Aux Messages :Reset All Cntrls | × | ○ (121) (*6) | |
| Aux Messages :Local On/Off | × | × | |
| Aux Messages :All Notes Off | × | ○ (123-127) (*6) | |
| Aux Messages :Active Sense | ○ | ○ | |
| Aux Messages :Reset | × | × | |

**Notes (EL Mode):**
- \*1: 1ch=UK, 2ch=LK, 3ch=PK, 16ch=CONTROL
- \*2: 1ch=UK, 2ch=LK, 3ch=PK, 4ch=LEAD1, 5-14ch=XG, 15ch=KEYBOARD PERCUSSION, 16ch=CONTROL
- \*3: UK:48-96, LK:36-96, PK:36-60(ELS-03XF), 36-55(ELS-03G/ELS-03X/ELS-03XR)
- \*4: UK, LK, PK, LEAD1: 36-96, XG:0-127, KEYBOARD PERCUSSION: 0-127
- \*5: UK=UK Horizontal Touch, LK=LK Horizontal Touch (ELS-03X, ELS-03XR, ELS-03XF)
- \*6: only XG
- \*7: only CONTROL
- \*8: only UK
- \*9: only in External Mode
- \*10: CONTROL: 0-15
- \*11: CONTROL: 0-15, XG: 0-127
- \*12: only ELS-03X/ELS-03XR/ELS-03XF

### XG Mode — YAMAHA [Electone-XG Mode]

対象: ELS-03XF, ELS-03XR, ELS-03X, ELS-03G  
送信は EL Mode と同じ

| Function | Transmitted (Same as EL Mode) | Recognized | Remarks |
|----------|-------------------------------|------------|---------|
| Basic Channel — Default | 1,2,3,16 | 1-16 | |
| Basic Channel — Changed | 1-16 | × | |
| Mode — Default | Mode 3 | Mode 3 | |
| Mode — Messages | × | × | |
| Mode — Altered | *************** | × | |
| Note Number | 36-96 | 0-127 | |
| Note Number :True voice | *************** | | |
| Velocity — Note on | ○ 9nH,v=1-127 | ○ 9nH,v=1-127 | |
| Velocity — Note off | × 9nH,v=0 | × 9nH,v=0 or 8nH | |
| After Touch — Key's | ○ (*1) | ○ | |
| After Touch — Ch's | ○ | ○ | |
| Pitch Bend | ○ | ○ | |
| Control Change 0,32 | × | ○ | Bank Select |
| Control Change 1,5,7,10 | × | ○ | |
| Control Change 4 | ○ | × | 2nd Expression |
| Control Change 6,38 | × | ○ | Data Entry |
| Control Change 11 | ○ | ○ | Expression |
| Control Change 16 | ○ (*1) | × | VA After Touch |
| Control Change 96,97 | × | ○ | Data Entry SW |
| Control Change 64-67 | × | ○ | |
| Control Change 71-78 | × | ○ | Sound Controller |
| Control Change 84,91,93,94 | × | ○ | |
| Control Change 98-99,100-101 | × | ○ | NRPN, RPN |
| Program Change | ○ 0-15 | ○ | |
| Program Change :True number | *************** | | |
| System Exclusive | ○ | ○ | |
| System Common :Song Position | × | × | |
| System Common :Song Select | × | × | |
| System Common :Tune | × | × | |
| System Real Time :Clock | ○ | × | |
| System Real Time :Commands | ○ | × | (FAH, FCH) |
| Aux Messages :All Sound Off | × | ○ (120) | |
| Aux Messages :Reset All Cntrls | × | ○ (121) | |
| Aux Messages :Local On/Off | × | × | |
| Aux Messages :All Notes Off | × | ○ (123-127) | |
| Aux Messages :Active Sense | ○ | ○ | |
| Aux Messages :Reset | × | × | |

**Notes (XG Mode):**
- \*1: only ELS-03X/ELS-03XR/ELS-03XF

---

```
Mode 1: OMNI ON,POLY    Mode 2: OMNI ON, MONO    ○: Yes
Mode 3: OMNI OFF,POLY   Mode 4: OMNI OFF,MONO    ×: No
```
