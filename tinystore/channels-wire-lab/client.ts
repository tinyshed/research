// Private research commands over the unchanged Bun SDK Session/Connection.
import {dlopen,FFIType,ptr} from 'bun:ffi'
import {readFileSync} from 'node:fs'
const sdk=process.env.SDK_ROOT!
const {Session}=await import(`${sdk}/src/session.ts`)
const {Connection}=await import(`${sdk}/src/connection.ts`)
const {bunRuntime}=await import(`${sdk}/src/runtime/bun.ts`)
const {credit}=await import(`${sdk}/src/wire/frame.ts`)
const [mode,algorithm,subscriberText,connectionText,profile='fast']=process.argv.slice(2)
const subscribers=Number(subscriberText),connectionCount=Number(connectionText)
const lib=dlopen(process.env.LAB_LIB!,{
 lab_open:{args:[FFIType.cstring,FFIType.bool],returns:FFIType.ptr},
 lab_connect:{args:[FFIType.ptr],returns:FFIType.ptr},
 lab_send:{args:[FFIType.ptr,FFIType.ptr,FFIType.usize],returns:FFIType.void},
 lab_recv:{args:[FFIType.ptr,FFIType.ptr,FFIType.usize],returns:FFIType.usize},
 lab_disconnect:{args:[FFIType.ptr],returns:FFIType.void},lab_close:{args:[FFIType.ptr],returns:FFIType.void},
})
const {symbols}=lib
const hub=mode==='embedded'?symbols.lab_open(ptr(Buffer.from(process.env.BENCH_DB!+'\0')),algorithm==='ring'):null
const pipes=new Set<any>(),buffer=new Uint8Array(256*1024)
const pump=setInterval(()=>{for(const p of pipes){const n=symbols.lab_recv(p.pointer,ptr(buffer),buffer.length);if(n)p.session.receive(buffer.subarray(0,Number(n)))}},1)
async function connect(){
 if(mode!=='embedded')return Connection.dial(bunRuntime,mode==='tcp'?'tcp://127.0.0.1:16979':`unix://${process.env.LAB_SOCKET}`,{client:'channels-research'})
 const pointer=symbols.lab_connect(hub);const session=new Session((bytes:Uint8Array)=>symbols.lab_send(pointer,ptr(bytes),bytes.length),{client:'channels-research'})
 const p={pointer,session};pipes.add(p)
 return Connection.overPipe(session,{write(bytes:Uint8Array){symbols.lab_send(pointer,ptr(bytes),bytes.length)},close(){if(pipes.delete(p))symbols.lab_disconnect(pointer)}})
}
async function call(c:any,method:number,body=new Uint8Array()){const stream=await c.session.open(method,body,true);const event=await stream.next();if(!event.end)throw Error('expected unary END');return event.body}
const memory=()=>({rss:process.memoryUsage().rss,heapUsed:process.memoryUsage().heapUsed,maxRss:process.resourceUsage().maxRSS})
const idle=memory(),connections:any[]=[]
for(let i=0;i<connectionCount;i++)connections.push(await connect())
const publisher=await connect()
const streams:any[]=[],delivered=Array(subscribers).fill(0),last=Array(subscribers).fill(-1),gaps=Array(subscribers).fill(0)
const latencies:number[]=[],tasks:Promise<void>[]=[];let orderingErrors=0,missed=0
for(let i=0;i<subscribers;i++){
 const c=connections[i%connections.length];const slow=profile==='slow'&&i%100===0
 // Voluntarily withhold the server's send allowance on selected streams. The
 // actual SDK/Wire CREDIT resumes them after the bounded log has overflowed.
 const body=new Uint8Array(4);new DataView(body.buffer).setUint32(0,slow?0:2<<20,true)
 const stream=await c.session.open(60000,body,true);await stream.next();streams.push({stream,c,slow})
 tasks.push((async()=>{try{while(true){const e=await stream.next();if(e.end)break;const bytes=e.body;const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength)
  if(bytes[bytes.length-1]===255){gaps[i]++;missed+=Number(view.getBigUint64(0,true))}
  else {const seq=Number(view.getBigUint64(0,true));if(seq<=last[i])orderingErrors++;last[i]=seq;delivered[i]++;if((delivered[i]&15)===0)latencies.push((performance.now()-view.getFloat64(8,true))*1000)}
  stream.consumed(bytes.length)
 } }catch(e){if(!(e as Error).message.includes('cancel'))throw e}})())
}
const subscribed=memory();const cpu0=process.cpuUsage();const started=performance.now()
const count=profile==='slow'?1200:1000,rate=profile==='slow'?400:200
const rollbacks=10
for(let i=0;i<rollbacks;i++)await call(publisher,60002,new Uint8Array(1024))
for(let seq=0;seq<count;seq++){
 const target=started+seq*1000/rate;const wait=target-performance.now();if(wait>0)await Bun.sleep(wait)
 const payload=new Uint8Array(1024).fill(7);const v=new DataView(payload.buffer);v.setBigUint64(0,BigInt(seq),true);v.setFloat64(8,performance.now(),true)
 await call(publisher,60001,payload)
}
const publishMs=performance.now()-started
if(profile==='slow')for(const {slow,c,stream} of streams)if(slow)c.session.post(credit(stream.id,2<<20))
await Bun.sleep(500)
const status=JSON.parse(new TextDecoder().decode(await call(publisher,60003)))
const cpu=process.cpuUsage(cpu0),endMemory=memory(),received=delivered.reduce((a,b)=>a+b,0)
for(const {stream} of streams)stream.cancel()
await Promise.all(tasks)
await Bun.sleep(50)
const afterCancel=JSON.parse(new TextDecoder().decode(await call(publisher,60003)))
// Reconnect establishes a new subscription at current tail, without replay.
const fresh=await connect(),stream=await fresh.session.open(60000,new Uint8Array(),true);await stream.next();let replay=false
const pending=stream.next().then(()=>{replay=true}).catch(()=>{})
await Bun.sleep(20);const noReplay=!replay;stream.cancel();await pending;await fresh.close()
for(const c of connections)await c.close();await publisher.close();clearInterval(pump);if(hub)symbols.lab_close(hub)
latencies.sort((a,b)=>a-b);const pct=(q:number)=>latencies[Math.ceil((latencies.length-1)*q)]??0
console.log(JSON.stringify({experiment:'channels-wire',mode,algorithm,profile,subscribers,connectionCount,payloadBytes:1024,offeredRate:rate,published:count,rollbackAttempts:rollbacks,publishMs,publishedPerS:count*1000/publishMs,delivered:received,expectedDeliveries:count*subscribers,lost:count*subscribers-received,deliveredBySubscriber:delivered,gapFrames:gaps.reduce((a,b)=>a+b,0),gapMissed:missed,orderingErrors,latency:{n:latencies.length,sampleEvery:16,p50Us:pct(.5),p99Us:pct(.99)},idleMemory:idle,subscribedMemory:subscribed,endMemory,cpuUs:cpu,status,afterCancel,noReplay,statusText:'pass'}))
