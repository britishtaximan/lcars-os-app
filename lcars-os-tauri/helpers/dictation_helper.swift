// LCARS OS — Dictation Helper (Cocoa App)
// Records speech and transcribes it using macOS native SFSpeechRecognizer.
// Reads config from ~/.lcars-os/dictation_config.txt (line 1: duration, line 2: output path).
// Writes partial results to <output_file>.partial, final to <output_file>.
// Stops early if <output_file>.stop is created.

import Cocoa
import Speech
import AVFoundation

class DictationDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        // Read config from file (more reliable than --args with 'open' command)
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let configPath = home + "/.lcars-os/dictation_config.txt"
        var duration: Double = 1800
        var outputPath = home + "/.lcars-os/dictation_result.txt"

        if let config = try? String(contentsOfFile: configPath, encoding: .utf8) {
            let lines = config.components(separatedBy: "\n")
            if lines.count >= 1, let d = Double(lines[0].trimmingCharacters(in: .whitespaces)) {
                duration = d
            }
            if lines.count >= 2 {
                let p = lines[1].trimmingCharacters(in: .whitespaces)
                if !p.isEmpty { outputPath = p }
            }
        }

        let errorPath = outputPath + ".err"
        let partialPath = outputPath + ".partial"
        let stopPath = outputPath + ".stop"

        // Clean old files
        try? FileManager.default.removeItem(atPath: outputPath)
        try? FileManager.default.removeItem(atPath: errorPath)
        try? FileManager.default.removeItem(atPath: partialPath)
        try? FileManager.default.removeItem(atPath: stopPath)

        SFSpeechRecognizer.requestAuthorization { status in
            guard status == .authorized else {
                let msg = "Speech recognition not authorized. Enable in System Settings > Privacy & Security > Speech Recognition."
                try? msg.write(toFile: errorPath, atomically: true, encoding: .utf8)
                DispatchQueue.main.async { NSApplication.shared.terminate(nil) }
                return
            }

            AVCaptureDevice.requestAccess(for: .audio) { granted in
                guard granted else {
                    let msg = "Microphone access denied. Enable in System Settings > Privacy & Security > Microphone."
                    try? msg.write(toFile: errorPath, atomically: true, encoding: .utf8)
                    DispatchQueue.main.async { NSApplication.shared.terminate(nil) }
                    return
                }

                guard let speechRecognizer = SFSpeechRecognizer(locale: Locale(identifier: "en-US")),
                      speechRecognizer.isAvailable else {
                    let msg = "Speech recognizer not available for en-US"
                    try? msg.write(toFile: errorPath, atomically: true, encoding: .utf8)
                    DispatchQueue.main.async { NSApplication.shared.terminate(nil) }
                    return
                }

                let audioEngine = AVAudioEngine()
                let request = SFSpeechAudioBufferRecognitionRequest()
                request.shouldReportPartialResults = true

                let inputNode = audioEngine.inputNode
                let recordingFormat = inputNode.outputFormat(forBus: 0)
                inputNode.installTap(onBus: 0, bufferSize: 4096, format: recordingFormat) { buffer, _ in
                    request.append(buffer)
                }

                do {
                    audioEngine.prepare()
                    try audioEngine.start()
                } catch {
                    let msg = "Audio engine failed: \(error.localizedDescription)"
                    try? msg.write(toFile: errorPath, atomically: true, encoding: .utf8)
                    DispatchQueue.main.async { NSApplication.shared.terminate(nil) }
                    return
                }

                try? "LISTENING".write(toFile: partialPath, atomically: true, encoding: .utf8)

                speechRecognizer.recognitionTask(with: request) { result, error in
                    if let result = result {
                        let transcript = result.bestTranscription.formattedString
                        if result.isFinal {
                            try? transcript.write(toFile: outputPath, atomically: true, encoding: .utf8)
                            try? FileManager.default.removeItem(atPath: partialPath)
                        } else {
                            try? transcript.write(toFile: partialPath, atomically: true, encoding: .utf8)
                        }
                    }
                    if error != nil {
                        if !FileManager.default.fileExists(atPath: outputPath) {
                            let msg = error?.localizedDescription ?? "Recognition error"
                            try? msg.write(toFile: errorPath, atomically: true, encoding: .utf8)
                        }
                    }
                }

                func stopAndExit() {
                    audioEngine.stop()
                    inputNode.removeTap(onBus: 0)
                    request.endAudio()

                    DispatchQueue.main.asyncAfter(deadline: .now() + 3.0) {
                        if !FileManager.default.fileExists(atPath: outputPath) &&
                           !FileManager.default.fileExists(atPath: errorPath) {
                            if let partial = try? String(contentsOfFile: partialPath, encoding: .utf8),
                               partial != "LISTENING" {
                                try? partial.write(toFile: outputPath, atomically: true, encoding: .utf8)
                            } else {
                                try? "".write(toFile: outputPath, atomically: true, encoding: .utf8)
                            }
                        }
                        try? FileManager.default.removeItem(atPath: partialPath)
                        try? FileManager.default.removeItem(atPath: stopPath)
                        NSApplication.shared.terminate(nil)
                    }
                }

                // Poll for stop signal — MUST be on main RunLoop
                DispatchQueue.main.async {
                    var elapsed: Double = 0
                    Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { timer in
                        elapsed += 1.0
                        if FileManager.default.fileExists(atPath: stopPath) || elapsed >= duration {
                            timer.invalidate()
                            stopAndExit()
                        }
                    }
                }
            }
        }
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.prohibited)
let delegate = DictationDelegate()
app.delegate = delegate
app.run()
