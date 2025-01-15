{
  pkgs ? import <nixpkgs> { },
}:
with pkgs;
mkShell {
  LD_LIBRARY_PATH = lib.makeLibraryPath [
    libxkbcommon
    libGL
    wayland
  ];
}
