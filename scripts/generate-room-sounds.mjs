// Original short, soft synthesized cues. Reproducible and license-free.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const destination=path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../quickshell/app/assets');
const voices={member_join:[620,830],member_leave:[520,390],self_join:[440,660,880],self_leave:[660,440,330],room_invite:[740,555,990]};
for(const [name,notes] of Object.entries(voices)) {
  const rate=48000,noteLength=0.115,total=Math.ceil((notes.length*noteLength+0.08)*rate);
  const wav=Buffer.alloc(44+total*2);
  wav.write('RIFF'); wav.writeUInt32LE(wav.length-8,4); wav.write('WAVEfmt ',8);
  wav.writeUInt32LE(16,16); wav.writeUInt16LE(1,20); wav.writeUInt16LE(1,22);
  wav.writeUInt32LE(rate,24); wav.writeUInt32LE(rate*2,28); wav.writeUInt16LE(2,32); wav.writeUInt16LE(16,34);
  wav.write('data',36); wav.writeUInt32LE(total*2,40);
  for(let i=0;i<total;i++) {
    const time=i/rate;
    let sample=0;
    notes.forEach((frequency,index)=>{
      const age=time-index*noteLength;
      if(age>=0 && age<noteLength+0.07) {
        const envelope=Math.min(1,age/0.012)*Math.exp(-age*22)*Math.min(1,(noteLength+0.07-age)/0.02);
        sample+=0.20*envelope*(Math.sin(2*Math.PI*frequency*age)+0.12*Math.sin(4*Math.PI*frequency*age));
      }
    });
    wav.writeInt16LE(Math.round(Math.max(-1,Math.min(1,sample))*32767),44+i*2);
  }
  fs.writeFileSync(path.join(destination,`${name}.wav`),wav);
}
