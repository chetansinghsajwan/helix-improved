# Our helix fork, built against the shared toolkit. Locked by the repo root flake.lock.
{
    inputs = {
        nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

        term-toolkit = {
            url = "path:../term-toolkit";
            inputs.nixpkgs.follows = "nixpkgs";
        };
    };

    outputs =
        {
            nixpkgs,
            term-toolkit,
            ...
        }:
        {
            packages = term-toolkit.lib.forEachSystem (
                sys:
                let
                    pkgs = nixpkgs.legacyPackages.${sys};
                in
                {
                    default = pkgs.callPackage ./default.nix {
                        craneLib = term-toolkit.lib.craneFor pkgs;
                        inherit (term-toolkit.lib) composeSrc;
                    };
                }
            );
        };
}
