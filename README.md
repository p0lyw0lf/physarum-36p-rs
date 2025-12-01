# physarum-36p-rs

Simplified wgpu implementation of [36 Points](https://www.sagejenson.com/36points), based on [Bleuje's implementation](https://github.com/Bleuje/physarum-36p).

Works best on fullscreen 1920x1080 window. Press F11 to toggle fullscreen.

## Keybinds

### Modifying Current Point Setting

All the values for the settings are displayed in the upper left. To modify a
given setting, press the key physically corresponding to it on left part of
your keyboard letters. On a US English keyboard, these keys are:

```
|---|---|---|---|---|
| Q | W | E | R | T |
|---|---|---|---|---|
| A | S | D | F | G |
|---|---|---|---|---|
| Z | X | C | V | B |
|---|---|---|---|---|
```

Once you select a parameter to modify, it will highlight green. Increment it
up/down with the up/down arrow keys. Change how much you're incrementing it by
with the left/right arrow keys.

To unselect a parameter, press the key for the parameter again, or press the
Escape key.

### Switch Between Default Point Settings

On a US English keyboard, press the left/right bracket keys `[]`. There will be
a number next to the point settings that show what preset you're on. If the
settings have been modified from the preset, a `*` will show next to the
indicator.

### Creating New Default Point Settings

* Enter: Save current settings as default for the selected preset.
* F1: Create new preset number, inserted after the current one.
* F5: Reset current settings to default for the preset.
* F9: Delete current preset.
* `/`: Randomize current settings.

### Automatically Modifying With Music

If the `--music` command line argument is provided and points to a valid MP3
file, this program will also play that file & display a live view of the
amplitudes of certain frequency bands in the top right. You can make the
amplitude of each of those bands individually apply changes to certain
parameters.

To select a frequency band, press the key physically corresponding to it on the
keyboard. On a US English keyboard, these are the keys:

```
|---|---|---|---|---|
| Y | U | I | O | P |
|---|---|---|---|---|
```

Once a band is selected, the level corresponding to it will turn red, as well
the settings to let you know what mode you're in. Then, modifying parameters
can be done like normal, only the currently selected parameter will be yellow
instead.

### Controlling Music Playback

Music playback is controlled with function keys that correspond to media keys
on my laptop. These will not be the same media keys on all laptops, but too bad
I'm writing this for me.

* F2: Seek backwards 10s
* F3: Play/Pause
* F4: Seek forwards 10s

There is no way to configure seek distance at this time.
