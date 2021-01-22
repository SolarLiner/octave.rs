import * as vscode from "vscode";

export interface Configuration {
  "octave.lspPath": string;
  "octave.launcher": string;
}

export class ConfigurationService {
  configuration: vscode.WorkspaceConfiguration;

  constructor(extensionId?: string) {
    this.configuration = vscode.workspace.getConfiguration(extensionId);
  }

  get<K extends keyof Configuration>(key: K): Configuration[K] {
    return this.configuration.get<Configuration[K]>(key)!;
  }

  has(key: keyof Configuration): boolean {
    return this.configuration.has(key);
  }

  inspect<K extends keyof Configuration>(key: K) {
    return this.configuration.inspect<Configuration[K]>(key);
  }
}
