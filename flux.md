[ [color=#83B806][b]Description[/b][/color] ]
» cs2-killtimer is a tiny external overlay for Counter-Strike 2 that shows a countdown timer between your kills. It uses read-only memory access, no DLL injection, and no hooks. The overlay sits as a transparent layer on top of the game and shows seconds since your last kill, with a color-banded streak indicator so you always know where you are in the streak window.

[ [color=#83B806][b]How It Works[/b][/color] ]
» When you get a kill, a timer appears on screen showing seconds since your last kill. The color tells you where you are in the streak window:

[list=*]
[*][color=#FF4040]Red[/color] while the streak is fresh[/*]
[*][color=#FFA040]Orange[/color] in the middle of the window[/*]
[*][color=#40C040]Green[/color] once the window has expired[/*]
[/list]

» The overlay never touches the game process beyond [b]ReadProcessMemory[/b], so it does not modify game state and does not interact with VAC-protected code paths.

[ [color=#83B806][b]Requirements[/b][/color] ]
[list=*]
[*]Windows 10 or 11, x64[/*]
[*]Counter-Strike 2[/*]
[/list]

[ [color=#83B806][b]Getting Started[/b][/color] ]

[h]Option 1: Download the binary[/h]
» Grab the latest [b]cs2-killtimer.exe[/b] from the [url=https://github.com/SamHoque/cs2-killtimer/releases]Releases page[/url].

[h]Option 2: Build from source[/h]
» Clone the repo and run:
[code]cargo build --release[/code]

» Output lands at:
[code]target/release/cs2-killtimer.exe[/code]

[h]Running[/h]
[list=*]
[*]Launch CS2[/*]
[*]Run [b]cs2-killtimer.exe[/b][/*]
[*]The overlay appears as a transparent layer on top of your game[/*]
[*]Close the console window to exit[/*]
[/list]

[ [color=#83B806][b]Showcase[/b][/color] ]
[video]https://youtu.be/b4H_NZLCcgU[/video]

[ [color=#83B806][b]Source[/b][/color] ]
» Repo: [url=https://github.com/SamHoque/cs2-killtimer]github.com/SamHoque/cs2-killtimer[/url]

[ [color=#83B806][b]Credits[/b][/color] ]
[list=*]
[*]Thanks to [b]@sweephvh[/b] for the idea[/*]
[*]Offsets courtesy of [url=https://github.com/a2x/cs2-dumper]cs2-dumper[/url][/*]
[*]Overlay text rasterization via [url=https://github.com/alexheretic/ab-glyph]ab_glyph[/url][/*]
[/list]
