import * as vscode from "vscode";

export class LogService implements vscode.Disposable {
  constructor(private channel: vscode.OutputChannel) {}

  log(message: string) {
    console.log(message);
    this.channel.appendLine(
      `[${new Date().toLocaleDateString()} ${new Date().toLocaleTimeString()}] ${message}`
    );
  }

  dispose() {
    this.channel.dispose();
  }
}
