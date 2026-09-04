// Synthetic local RGBA frames only. No Wisp account, server, capture, or clipboard.
import net from 'node:net';
const header=Buffer.alloc(8); header.writeUInt32LE(640); header.writeUInt32LE(240,4);
const pixels=Buffer.alloc(640*240*4);
for(let y=0;y<240;y++)for(let x=0;x<640;x++) {
  const p=(y*640+x)*4;
  pixels[p]=x<320?44:160; pixels[p+1]=y<120?140:70; pixels[p+2]=120; pixels[p+3]=255;
}
net.createServer(socket=>{
  socket.on('error',()=>{});
  let input='',handshake=false;
  socket.on('data',data=>{
    input+=data.toString();
    for(let index;(index=input.indexOf('\n'))>=0;) {
      const line=input.slice(0,index); input=input.slice(index+1);
      if(!handshake) { JSON.parse(line); handshake=true; continue; }
      socket.write(header); socket.write(pixels);
    }
  });
}).listen(process.argv[2]);
