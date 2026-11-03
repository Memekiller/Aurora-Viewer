# Rain sounds

The loops of the rain (the rain prototype's ambience): field recordings from
Freesound, cut, levelled and converted for Aurora.

| File | Recording | Licence |
|---|---|---|
| `rain-light.ogg` | "Light Rain" by Jeffreys2, https://freesound.org/people/Jeffreys2/sounds/137022/ | CC0 1.0 |
| `rain-heavy.ogg` | "Ambiance Heavy Rain Loop" by DWOBoyle, https://freesound.org/people/DWOBoyle/sounds/136971/ | CC BY 4.0 |
| `rain-roof.ogg` | "Rain Indoor Cabin (loop)" by PhilllChabbb, https://freesound.org/people/PhilllChabbb/sounds/232731/ | CC0 1.0 |
| `thunder-*.ogg` | "Three mighty thunders in row" by Nimlos, https://freesound.org/people/Nimlos/sounds/522025/ | CC0 1.0 |

"Ambiance Heavy Rain Loop" is by DWOBoyle, licensed under Creative Commons
Attribution 4.0 (https://creativecommons.org/licenses/by/4.0/); changed as
below.

Changes: all are 48 kHz stereo Ogg Vorbis (quality 4), brought toward an RMS
level of -24 dBFS, peaks kept under -1 dBFS (light: -24.0 dBFS, heavy:
-30.2 dBFS, roof: -24.0 dBFS; the player makes up the difference). The light
loop is 60 s built from random pieces (1 to 2 s, from 0.8 s in) of its 7 s
recording, each crossfaded into the next over 0.4 s (equal power), the last
into the first, so that it loops without a seam or a short repeat; its
rumble below 150 Hz is taken out (second-order high-pass). The roof loop has
its rumble below 80 Hz taken out (second-order high-pass).

The thunder clips are five pieces of that one recording, 48 kHz mono Ogg
Vorbis (quality 5): `thunder-far` (1 to 12 s), `thunder-crack-1` (28.2 s,
18 s long), `thunder-crack-2` (60.7 s, 14 s), `thunder-double` (94.7 s,
18 s) and `thunder-roll` (125.5 s, 13 s). Its steady background (wind) is
taken out by noise reduction from a quiet stretch (77 to 82 s; the roll,
from the rain alone, 116 to 124 s), the rumble below 25 Hz by a high-pass;
each fades in over 20 ms and out over its last 4 to 6 s, peaks at -1 dBFS.
