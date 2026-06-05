{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-25.11";
    fenix = {
      url = "fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        toolchain = fenix.packages.${system}.stable.withComponents [
          "cargo"
          "clippy"
          "rust-analyzer"
          "rust-src"
          "rustc"
          "rustfmt"
        ];

        platform = pkgs.makeRustPlatform {
          cargo = toolchain;
          rustc = toolchain;
        };

        cargoToml = pkgs.lib.importTOML ./Cargo.toml;
      in
      {
        packages.default = platform.buildRustPackage rec {
          pname = cargoToml.package.name;
          version = cargoToml.package.version;
          description = cargoToml.package.description;

          buildInputs = with pkgs; [ openssl ];
          nativeBuildInputs = with pkgs; [ pkg-config ];

          src = pkgs.lib.cleanSource ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          # Build only the library and the pretty_print example
          cargoBuildFlags = [
            "--lib"
            "--example"
            "pretty_print"
          ];

          postInstall = ''
            install -Dm755 target/*/release/examples/pretty_print $out/bin/claus-pretty-print
          '';
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ self.packages.${system}.default ];
          buildInputs = [ pkgs.nixfmt-rfc-style ];
        };

        packages.docker = pkgs.dockerTools.buildImage {
          name = cargoToml.package.name;
          tag = cargoToml.package.version;

          config = {
            Cmd = [ "${self.packages.${system}.default}/bin/${cargoToml.package.name}" ];
          };
        };
      }
    );
}
