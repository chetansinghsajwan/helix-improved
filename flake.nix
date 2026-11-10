# helix-improved: our helix fork, built with crane. The shared toolkit comes from the `tuikit`
# submodule (path dependencies in helix-term), so clone with `--recurse-submodules`.
{
    inputs = {
        nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

        # Splits cargo builds into a cached dependency build and our code.
        crane.url = "github:ipetkov/crane";
    };

    outputs =
        { nixpkgs, crane, ... }:
        let
            forEachSystem = nixpkgs.lib.genAttrs [
                "x86_64-linux"
                "aarch64-linux"
                "x86_64-darwin"
                "aarch64-darwin"
            ];

            # The part of this repo the build needs, so editing anything else does not rebuild it.
            # Never local build output (target/) or the fetched grammar sources.
            sourceOf =
                lib:
                lib.cleanSourceWith {
                    src = ./.;
                    filter =
                        path: _:
                        let
                            rel = lib.removePrefix "${toString ./.}/" (toString path);
                            inTuikit = rel == "tuikit" || rel == "tuikit/Cargo.toml" || rel == "tuikit/crates" || lib.hasPrefix "tuikit/crates/" rel;
                        in
                        (
                            rel == (toString ./.)
                            || lib.any (d: rel == d || lib.hasPrefix "${d}/" rel) [
                                ".cargo"
                                "helix-core"
                                "helix-dap"
                                "helix-dap-types"
                                "helix-event"
                                "helix-loader"
                                "helix-lsp"
                                "helix-lsp-types"
                                "helix-parsec"
                                "helix-stdx"
                                "helix-term"
                                "helix-tui"
                                "helix-vcs"
                                "helix-view"
                                "runtime"
                                "xtask"
                            ]
                            || builtins.elem rel [
                                "Cargo.toml"
                                "Cargo.lock"
                                "rust-toolchain.toml"
                                "rustfmt.toml"
                                "languages.toml"
                                "theme.toml"
                                "base16_theme.toml"
                            ]
                            || inTuikit
                        )
                        && baseNameOf path != "target"
                        && !(lib.hasSuffix "runtime/grammars/sources" rel);
                };
        in
        {
            packages = forEachSystem (
                sys:
                let
                    pkgs = nixpkgs.legacyPackages.${sys};
                    helix = pkgs.callPackage ./default.nix {
                        craneLib = crane.mkLib pkgs;
                        src = sourceOf pkgs.lib;
                    };
                in
                {
                    inherit helix;
                    default = helix;
                }
            );
        };
}
