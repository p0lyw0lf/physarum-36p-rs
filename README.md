# physarum-36p-rs

Simplified wgpu implementation of [36 Points](https://www.sagejenson.com/36points), based on [Bleuje's implementation](https://github.com/Bleuje/physarum-36p).

Works best on fullscreen 1920x1080 window. Press F11 to toggle fullscreen.

TODO:
+ [x] Ability to modify current point settings
+ [x] Ability to switch between default point settings
+ [x] Ability to sync changes in point settings to different parts of the
      music's frequency band amplitudes.
+ [ ] Ability to play/pause music
+ [ ] Ability to save/load default point settings

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
