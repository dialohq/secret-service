{
  pkgs,
  lib,
  config,
  secret-service,
  ...
}: let
  cfg = config.dialo.services.secretservice;
  t = lib.types;
in {
  options.dialo.services.secretservice = {
    enable = lib.mkEnableOption "secretservice";
    dependants = lib.mkOption {
      type = t.listOf t.str;
      default = [];
    };
    target = lib.mkOption {
      type = t.str;
      description = "Address (host:port) the client targets, e.g. broadcast or server IP";
    };
    bind = lib.mkOption {
      type = t.str;
      example = "192.168.100.101";
    };
    secrets = lib.mkOption {
      default = {};
      type = t.attrsOf (
        t.submodule {
          options = {
            type = lib.mkOption {type = t.enum ["command"];};
            command = lib.mkOption {
              type = t.nullOr t.str;
              default = null;
            };
            targetPath = lib.mkOption {type = t.str;};
          };
        }
      );
    };
    hosts = lib.mkOption {
      default = {};
      description = "Hosts the server will deliver secrets to, keyed by hostname";
      type = t.attrsOf (
        t.submodule {
          options = {
            access = lib.mkOption {
              type = t.submodule {
                options = {
                  ssh = lib.mkOption {
                    type = t.submodule {
                      options = {
                        username = lib.mkOption {
                          type = t.str;
                          default = "root";
                        };
                        address = lib.mkOption {type = t.str;};
                        key = lib.mkOption {
                          type = t.submodule {
                            options = {
                              type = lib.mkOption {type = t.str;};
                              command = lib.mkOption {type = t.str;};
                              targetPath = lib.mkOption {
                                type = t.str;
                                default = "-";
                                readOnly = true;
                              };
                            };
                          };
                        };
                      };
                    };
                  };
                };
              };
            };
          };
        }
      );
    };
    server = lib.mkOption {
      default = {};
      type = t.submodule {
        options = {
          enable = lib.mkEnableOption "SecretService Server";
          port = lib.mkOption {
            type = t.str;
            default = "41234";
          };
          root = lib.mkOption {
            type = t.bool;
            default = false;
          };
          credentials = lib.mkOption {
            default = [];
            type = t.listOf (
              t.submodule {
                options = {
                  name = lib.mkOption {type = t.str;};
                  filePath = lib.mkOption {type = t.str;};
                  encrypted = lib.mkOption {
                    type = t.bool;
                    default = false;
                  };
                };
              }
            );
          };
          path = lib.mkOption {
            type = t.listOf t.package;
            default = [];
          };
        };
      };
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.secret-service-client = {
      description = "Secret service client";
      before = ["multi-user.target"];

      wants = [
        "network-online.target"
        "secret-service-server.service"
      ];
      after = [
        "network-online.target"
        "secret-service-server.service"
      ];
      unitConfig = {
        StartLimitBurst = 3;
        StartLimitIntervalSec = 60;
      };
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${secret-service}/bin/secretservice client --target ${cfg.target} --bind ${cfg.bind}";
        Restart = "on-failure";
        RestartSec = "5s";
        TimeoutStopSec = "5s";
        TimeoutStartSec = "30s";
      };
    };

    systemd.services.secret-service-server = lib.mkIf cfg.server.enable {
      description = "Secret service server";
      before = ["multi-user.target"];

      wants = ["network-online.target"];
      after = ["network-online.target"];
      path = cfg.server.path ++ [pkgs.bash];
      unitConfig = {
        StartLimitBurst = 3;
        StartLimitIntervalSec = 60;
      };
      serviceConfig = {
        DynamicUser = "yes";
        PrivateTmp = true;
        LoadCredential = map (c: "${c.name}:${c.filePath}") cfg.server.credentials;
        StateDirectory = "secret-service-server-state";
        RuntimeDirectory = "secret-service-server-runtime";
        Type = "exec";
        ExecStart = "${secret-service}/bin/secretservice server --port ${cfg.server.port} ${
          if cfg.server.root
          then "--root"
          else ""
        } --config '${pkgs.writeText "config" (builtins.toJSON {inherit (cfg) secrets hosts;})}'";
        Restart = "on-failure";
        TimeoutStopSec = "5s";
        RestartSec = "60s";
      };
    };
  };
}
