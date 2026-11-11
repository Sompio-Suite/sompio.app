// Collects microphone audio into 20 ms frames (960 samples at 48 kHz) for the Opus encoder.
class VoiceCapture extends AudioWorkletProcessor {
  constructor() {
    super();
    this.buf = new Float32Array(960);
    this.n = 0;
  }
  process(inputs) {
    const ch = inputs[0] && inputs[0][0];
    if (!ch) return true;
    for (let i = 0; i < ch.length; i++) {
      this.buf[this.n++] = ch[i];
      if (this.n === 960) {
        let sum = 0;
        for (let j = 0; j < 960; j++) sum += this.buf[j] * this.buf[j];
        const frame = this.buf;
        this.port.postMessage({ frame, level: Math.sqrt(sum / 960) }, [frame.buffer]);
        this.buf = new Float32Array(960);
        this.n = 0;
      }
    }
    return true;
  }
}
registerProcessor("voice-capture", VoiceCapture);
