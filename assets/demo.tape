Output "demo.gif"

Require echo

Set Shell "zsh"
Set FontSize 14
Set Width 1200
Set Height 600
Set TypingSpeed 50ms
Set FontFamily "Hack Nerd Font"
Set Padding 16
Set WindowBar Colorful
Set Framerate 20

Hide
Type "clear"
Enter
Show

Type `cat ../README.md | mq-edit | mq -A '.h | first()'`
Enter
Sleep 1.5s

Type `# Welcome to the demo of mq-edit!`
Enter 2
Sleep 1.5s

Down 15
Sleep 0.5s

PageDown 2
Sleep 0.5s

Down 15
Sleep 0.5s

Escape
Sleep 2s
