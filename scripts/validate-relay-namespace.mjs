import fs from 'node:fs';
import path from 'node:path';
const supplied=Object.prototype.hasOwnProperty.call(process.env,'VITE_HARBOR_RELAY_NAMESPACE');
const value=(supplied ? process.env.VITE_HARBOR_RELAY_NAMESPACE : 'harbor.social').trim();
const valid=/^(?=.{4,253}$)(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(value);
if(!valid){console.error(`Invalid VITE_HARBOR_RELAY_NAMESPACE: ${JSON.stringify(value)}`);process.exit(1)}
if(process.argv.includes('--dist')){const root='dist';if(!fs.existsSync(root)){console.error('dist is missing');process.exit(1)}const text=fs.readdirSync(root,{recursive:true}).filter(f=>/\.(js|html|css)$/.test(f)).map(f=>fs.readFileSync(path.join(root,f),'utf8')).join('\n');if(text.includes('@name@relay')){console.error('Packaged frontend contains fabricated @name@relay fallback');process.exit(1)}if(!text.includes(value)){console.error(`Packaged frontend does not contain configured namespace ${value}`);process.exit(1)}}
console.log(`Relay namespace validated: ${value}`);
