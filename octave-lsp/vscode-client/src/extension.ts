// The module 'vscode' contains the VS Code extensibility API
// Import the module and reference it with the alias vscode in your code below
import * as vscode from "vscode";
import * as path from "path";
import * as fs from "fs";
import "reflect-metadata";
import { LogService } from "./services/logger";
import { ConfigurationService } from "./services/configuration";
import {
  Disposable,
  Executable,
  LanguageClient,
  Trace,
  TransportKind,
} from "vscode-languageclient";

// this method is called when your extension is activated
// your extension is activated the very first time the command is executed
export function activate(context: vscode.ExtensionContext) {
  context.subscriptions.push(new Extension(context));
}

// this method is called when your extension is deactivated
export function deactivate() {}

class Extension implements Disposable {
  private readonly client: LanguageClient;
  private readonly config: ConfigurationService;
  private readonly logger: LogService;

  constructor(private readonly context: vscode.ExtensionContext) {
    this.logger = new LogService(vscode.window.createOutputChannel("Octave"));
    this.config = new ConfigurationService();
    const cmd: Executable = {
      command: findBinary(this.config.get("octave.lspPath")),
      options: {
        // eslint-disable-next-line @typescript-eslint/naming-convention
        env: { RUST_BACKTRACE: "1" },
      },
    };
    this.client = new LanguageClient(
      "octave-lsp",
      "Octave LSP",
      {
        run: cmd,
        debug: cmd,
      },
      { documentSelector: [{ scheme: "file", language: "octave" }] }
    );
    this.client.trace = Trace.Verbose;
    this.client.traceOutputChannel.show(true);
    this.client.start();
    this.logger.log("Octave extension activated");
  }

  dispose() {
    this.logger.log("Deactivating Octave extension");
    this.logger.dispose();
    this.client.stop();
  }
}
function findBinary(pathOrName: string): string {
  if (path.isAbsolute(pathOrName)) return pathOrName;
  else
    for (const dir of process.env.PATH?.split(path.delimiter) ?? []) {
      const absolute = path.join(dir, pathOrName);
      if (fs.existsSync(absolute)) return absolute;
    }

  throw new Error(`Server at "${pathOrName} not found`);
}
