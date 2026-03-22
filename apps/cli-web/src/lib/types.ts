export interface OptionDoc {
  flags: string;
  value_name?: string;
  help?: string;
  default?: string;
  required: boolean;
  is_flag: boolean;
}

export interface ArgumentDoc {
  name: string;
  help?: string;
  default?: string;
  required: boolean;
}

export interface CommandDoc {
  name: string;
  about?: string;
  synopsis: string;
  global_options?: OptionDoc[];
  options?: OptionDoc[];
  arguments?: ArgumentDoc[];
  subcommands?: CommandDoc[];
}
