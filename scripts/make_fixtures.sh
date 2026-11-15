#!/usr/bin/env bash
# Generates test media in ./fixtures. Each 5.1 channel carries its own tone, so
# you can hear (and see on the meters) exactly which channels are selected:
#   FL 300 Hz · FR 500 Hz · C 700 Hz · LFE 60 Hz · SL 1100 Hz · SR 1300 Hz
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p fixtures
D=${DURATION:-30}
ff() { ffmpeg -hide_banner -loglevel error -y "$@"; }

ff -f lavfi -i "sine=frequency=300:duration=$D" -f lavfi -i "sine=frequency=500:duration=$D" \
   -f lavfi -i "sine=frequency=700:duration=$D" -f lavfi -i "sine=frequency=60:duration=$D" \
   -f lavfi -i "sine=frequency=1100:duration=$D" -f lavfi -i "sine=frequency=1300:duration=$D" \
   -filter_complex "[0][1][2][3][4][5]join=inputs=6:channel_layout=5.1(side):map=0.0-FL|1.0-FR|2.0-FC|3.0-LFE|4.0-SL|5.0-SR[a]" \
   -map "[a]" -ar 48000 -c:a pcm_s24le fixtures/tones_51.wav

ff -i fixtures/tones_51.wav -c:a ac3 -b:a 640k fixtures/tones_51.ac3
ff -i fixtures/tones_51.wav -c:a dca -strict -2 -b:a 1509k fixtures/tones_51.dts
ff -i fixtures/tones_51.wav -c:a aac -b:a 512k fixtures/tones_51.m4a
ff -i fixtures/tones_51.wav -c:a flac fixtures/tones_51.flac
ff -i fixtures/tones_51.wav -af "pan=stereo|FL=FL|FR=FR" -c:a pcm_s16le fixtures/tones_20.wav
# Movie-style file: video + DTS 5.1 + a stereo commentary track; audio starts 0.5 s late.
ff -f lavfi -i "testsrc2=size=1280x720:rate=24:duration=$((D + 1))" -itsoffset 0.5 -i fixtures/tones_51.wav \
   -i fixtures/tones_20.wav -map 0:v -map 1:a -map 2:a -c:v libx264 -preset veryfast \
   -c:a:0 dca -strict -2 -b:a:0 1509k -c:a:1 aac -b:a:1 192k \
   -metadata:s:a:0 language=eng -metadata:s:a:0 title="Main 5.1" \
   -metadata:s:a:1 language=eng -metadata:s:a:1 title="Commentary" fixtures/movie_dts51.mkv
ls -la fixtures
