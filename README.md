# Nifi-runner

Note: Very experimental!

Cli application that interacts with a nifi-instance, powered by linked data.

## Usage

Listing all currently supported component types: `./nifi-runner list types`.
Listing all currently supported service types: `./nifi-runner list services`.
Listing properties for a specific component: `./nifi-runner list type <TYPE-STRING>`.
Listing properties for a specific service: `./nifi-runner list service <TYPE-STRING>`.

### Create a nifi component in a nifi instance

```shell
./nifi-runner list type org.apache.nifi.processors.standard.PostHTTP > ontology.ttl

cat > input.ttl << EOF
@base <http://example.com/ns#>.
@prefix : <https://w3id.org/conn#> .

[] a :NifiChannel;
  :reader _:b2;
  :writer _:b1.

_:b1 a :NifiWriterChannel.
_:b2 a :NifiReaderChannel.

_:b3 a <PostHTTP>;
  <success> _:b1;  
  <INCOMING_CHANNEL> _:b2;
  <URL> "localhost:3000/posting".
EOF 

./nifi-runner run -o ontology.ttl input.ttl
```

Congratulations you now have an HTTP Post component that if the post is successful will post a new message!



