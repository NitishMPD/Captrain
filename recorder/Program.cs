using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text;
using System.Threading;
using System.Web.Script.Serialization;
using ScreenRecorderLib;

// This process owns devices and textures. Only configuration and status cross its pipes.
internal static class Program
{
    private static readonly object OutputLock = new object();
    private static readonly JavaScriptSerializer Json = new JavaScriptSerializer();
    private static void Send(object value)
    {
        lock (OutputLock) { Console.WriteLine(Json.Serialize(value)); Console.Out.Flush(); }
    }

    private sealed class Request
    {
        public string operation { get; set; }
        public string path { get; set; }
        public string device { get; set; }
        public long window { get; set; }
        public int x { get; set; }
        public int y { get; set; }
        public int width { get; set; }
        public int height { get; set; }
        public int outputWidth { get; set; }
        public int outputHeight { get; set; }
        public int fps { get; set; }
        public int bitrate { get; set; }
        public bool cursor { get; set; }
        public bool systemAudio { get; set; }
        public string microphone { get; set; }
        public string camera { get; set; }
        public string corner { get; set; }
        public int cameraSize { get; set; }
    }

    [STAThread]
    private static int Main()
    {
        try
        {
            Console.InputEncoding = new UTF8Encoding(false);
            Console.OutputEncoding = new UTF8Encoding(false);
            var request = Json.Deserialize<Request>(Console.ReadLine() ?? "{}");
            if (request.operation == "devices")
            {
                Send(new {
                    microphones = Recorder.GetSystemAudioCaptureDevices().Select(d => new { id = d.DeviceName, name = d.FriendlyName }),
                    cameras = Recorder.GetSystemVideoCaptureDevices().Select(d => new { id = d.DeviceName, name = d.FriendlyName })
                });
                return 0;
            }
            if (request.operation != "record" || request.width < 2 || request.height < 2 ||
                request.outputWidth < 2 || request.outputHeight < 2 ||
                (request.fps != 30 && request.fps != 60) || request.bitrate < 1000000)
                throw new ArgumentException("Invalid recording configuration.");

            RecordingSourceBase source;
            if (request.window != 0)
                source = new WindowRecordingSource(new IntPtr(request.window)) {
                    IsCursorCaptureEnabled = request.cursor, IsBorderRequired = true
                };
            else
                source = new DisplayRecordingSource(request.device) {
                    RecorderApi = RecorderApi.WindowsGraphicsCapture,
                    IsCursorCaptureEnabled = request.cursor, IsBorderRequired = true,
                    SourceRect = new ScreenRect(request.x, request.y, request.width, request.height)
                };

            var audio = new List<AudioSourceBase>();
            if (request.systemAudio)
                audio.Add(LoopbackAudioSource.Default ?? throw new InvalidOperationException("No system audio output is available."));
            if (request.microphone != null)
            {
                var device = Recorder.GetSystemAudioCaptureDevices().FirstOrDefault(d => d.DeviceName == request.microphone);
                audio.Add(device ?? throw new InvalidOperationException("The selected microphone is unavailable."));
            }
            var overlays = new List<RecordingOverlayBase>();
            if (request.camera != null)
            {
                if (!Recorder.GetSystemVideoCaptureDevices().Any(d => d.DeviceName == request.camera))
                    throw new InvalidOperationException("The selected camera is unavailable.");
                var size = Math.Max(32, Math.Min(request.width, request.height) * request.cameraSize / 100);
                overlays.Add(new VideoCaptureOverlay {
                    DeviceName = request.camera, Size = new ScreenSize(size, size),
                    AnchorPoint = (Anchor)Enum.Parse(typeof(Anchor), request.corner),
                    Offset = new ScreenSize(16, 16), Stretch = StretchMode.UniformToFill
                });
            }
            var options = new RecorderOptions {
                SourceOptions = new SourceOptions { RecordingSources = new List<RecordingSourceBase> { source } },
                OutputOptions = new OutputOptions {
                    RecorderMode = RecorderMode.Video, OutputFrameSize = new ScreenSize(request.outputWidth, request.outputHeight),
                    Stretch = StretchMode.Uniform, IsVideoFramePreviewEnabled = false
                },
                VideoEncoderOptions = new VideoEncoderOptions {
                    Encoder = new H264VideoEncoder { EncoderProfile = H264Profile.High, BitrateMode = H264BitrateControlMode.CBR },
                    Framerate = request.fps, Bitrate = request.bitrate,
                    // shortcut: Windows selects the transform; verify the actual encoder before promising hardware-only recording.
                    IsFixedFramerate = true, IsHardwareEncodingEnabled = true,
                    IsLowLatencyEnabled = true, IsThrottlingDisabled = false,
                    IsFragmentedMp4Enabled = true, IsMp4FastStartEnabled = false
                },
                AudioOptions = new AudioOptions { IsAudioEnabled = audio.Count > 0, AudioSources = audio, Channels = AudioChannels.Stereo, Bitrate = AudioBitrate.bitrate_192kbps },
                OverlayOptions = new OverLayOptions { Overlays = overlays },
                MouseOptions = new MouseOptions { IsMousePointerEnabled = request.cursor, IsMouseClicksDetected = false },
                SnapshotOptions = new SnapshotOptions { SnapshotsWithVideo = false }
            };

            using (var recorder = Recorder.CreateRecorder(options))
            using (var finished = new ManualResetEventSlim())
            {
                var clock = new Stopwatch();
                string status = "starting";
                bool failed = false;
                recorder.OnStatusChanged += (_, e) => {
                    lock (OutputLock)
                    {
                        status = e.Status.ToString().ToLowerInvariant();
                        if (e.Status == RecorderStatus.Recording) clock.Start(); else clock.Stop();
                        Send(new { status, elapsedMs = clock.ElapsedMilliseconds, frames = Math.Max(0, recorder.CurrentFrameNumber) });
                    }
                };
                recorder.OnRecordingComplete += (_, e) => {
                    Send(new { status = "complete", elapsedMs = clock.ElapsedMilliseconds, frames = Math.Max(0, recorder.CurrentFrameNumber) });
                    finished.Set();
                };
                recorder.OnRecordingFailed += (_, e) => {
                    failed = true;
                    Send(new { status = "error", error = e.Error });
                    finished.Set();
                };
                recorder.Record(request.path);
                var controls = new Thread(() => {
                    try
                    {
                        string command;
                        while ((command = Console.ReadLine()) != null && !finished.IsSet)
                        {
                            if (command == "pause") recorder.Pause();
                            else if (command == "resume") recorder.Resume();
                            else if (command == "stop") { recorder.Stop(); return; }
                        }
                        if (!finished.IsSet) recorder.Stop(); // Parent death closes stdin: finalize the file.
                    }
                    catch (Exception e) { Send(new { status = "error", error = e.Message }); finished.Set(); }
                }) { IsBackground = true };
                controls.Start();
                while (!finished.Wait(1000))
                {
                    lock (OutputLock) Send(new { status, elapsedMs = clock.ElapsedMilliseconds, frames = Math.Max(0, recorder.CurrentFrameNumber) });
                }
                return failed ? 1 : 0;
            }
        }
        catch (Exception e) { Send(new { status = "error", error = e.Message }); return 1; }
    }
}
