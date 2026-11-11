set -u
D="ddcutil --bus 8 --sleep-multiplier 2"
read_state() {
  echo "--- state"
  $D --terse getvcp 0x60 0xf2 0xf4 0x95 0x96 0x97 0x98 2>&1 | sed 's/^/  /'
}
read_state
echo "A) PIP without swap: F2=0x0100"
$D setvcp 0xf2 0x0100; sleep 7
read_state
echo "B) select window and set HDMI1"
$D setvcp 0xa5 0x0100; sleep 7
$D setvcp 0x60 0x12; sleep 5
read_state
echo "C) restore single USB-C"
$D setvcp 0xa5 0x0100; sleep 4
$D setvcp 0x60 0x31; sleep 4
$D setvcp 0xf2 0x0000; sleep 4
$D setvcp 0xa5 0x0000; sleep 3
read_state
