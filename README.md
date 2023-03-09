# cleansweeper

A variant of minesweeper, where the number represents how many unflagged mines are left,
not the total number of mines.
Chording is performed automatically.

Left click to flag, right click to open.

Command line options to configure the board:

    Options:
      -h, --height <HEIGHT>      Height of Cleansweeper grid - default 16
      -w, --width <WIDTH>        Width of Cleansweeper grid - default 16
      -f, --fraction <FRACTION>  Fraction of cells which contain bombs - default 0.25
