const YAML = require('yaml');
const fs = require('fs');
const path = require('path');
const readline = require('readline-sync');
const shell = require('shelljs');
const { Keyring } = require('@polkadot/api');
const { cryptoWaitReady, encodeAddress, decodeAddress } = require('@polkadot/util-crypto');
const _ = require('lodash');

const yargs = require('yargs/yargs');
const { hideBin } = require('yargs/helpers');

const checkOverrideFile = (path, yes) => {
  if (fs.existsSync(path) && !yes) {
    const res = readline.keyInYN(`'${path}' alraedy exists. Do you wish to override it?`);
    if (!res) {
      console.log('Bailing... Bye.');
      process.exit(0);
      return;
    }
  }
};

const exec = (cmd, fatal = true) => {
  console.log(`$ ${cmd}`);
  const res = shell.exec(cmd, { silent: true });
  if (res.code !== 0) {
    console.error('Error: Command failed with code', res.code);
    if (fatal) {
      process.exit(1);
    }
  }
  return res;
};

const fatal = (...args) => {
  console.trace('Error:', ...args);
  process.exit(1);
};

const getChainspec = (image, chain) => {
  const res = exec(
    `docker run --rm ${image} build-spec --chain=${chain} --disable-default-bootnode`
  );

  let spec;

  try {
    spec = JSON.parse(res.stdout);
  } catch (e) {
    return fatal('build spec failed', e);
  }

  return spec;
};

const exportParachainGenesis = (paraConfig, output) => {
  if (!paraConfig.image) {
    return fatal('Missing parachains[].image');
  }

  const args = [];
  if (paraConfig.chain) {
    args.push(`--chain=/app/${paraConfig.chain.base || paraConfig.chain}-${paraConfig.id}.json`);
  }

  const res2 = exec(`docker run -v $(pwd)/"${output}":/app --rm ${paraConfig.image} export-genesis-wasm ${args.join(' ')}`);
  const wasm = res2.stdout.trim();

  if (paraConfig.id) {
    args.push(`--parachain-id=${paraConfig.id}`);
  }
  const res = exec(`docker run -v $(pwd)/"${output}":/app --rm ${paraConfig.image} export-genesis-state ${args.join(' ')}`);
  const state = res.stdout.trim();

  return { state, wasm };
};

const generateRelaychainGenesisFile = (config, relaychainGenesisFilePath, output) => {
  const relaychain = config.relaychain;
  if (!relaychain) {
    return fatal('Missing relaychain');
  }
  if (!relaychain.chain) {
    return fatal('Missing relaychain.chain');
  }
  if (!relaychain.image) {
    return fatal('Missing relaychain.image');
  }
  const spec = getChainspec(relaychain.image, relaychain.chain);

  // clear authorities

  const runtime = spec.genesis.runtime.runtime_genesis_config || spec.genesis.runtime;

  const sessionKeys = runtime.session.keys;
  sessionKeys.length = 0;

  // add authorities from config
  const keyring = new Keyring();
  for (const { name } of config.relaychain.nodes) {
    const srAcc = keyring.createFromUri(`//${_.startCase(name)}`, null, 'sr25519');
    const srStash = keyring.createFromUri(`//${_.startCase(name)}//stash`, null, 'sr25519');

    const edAcc = keyring.createFromUri(`//${_.startCase(name)}`, null, 'ed25519');

    const ecAcc = keyring.createFromUri(`//${_.startCase(name)}`, null, 'ecdsa');

    let key = [
      srStash.address,
      srStash.address,
      {
        grandpa: edAcc.address,
        babe: srAcc.address,
        im_online: srAcc.address,
        parachain_validator: srAcc.address,
        authority_discovery: srAcc.address,
        para_validator: srAcc.address,
        para_assignment: srAcc.address,
        beefy: encodeAddress(ecAcc.publicKey),
      },
    ];

    sessionKeys.push(key);
  }

  // additional patches
  if (config.relaychain.runtime_genesis_config) {
    _.merge(runtime, config.relaychain.runtime_genesis_config);
  }

  // genesis parachains
  for (const parachain of config.paras) {
    const { wasm, state } = exportParachainGenesis(parachain, output);
    if (!parachain.id) {
      return fatal('Missing parachains[].id');
    }
    const para = [
      parachain.id,
      {
        genesis_head: state,
        validation_code: wasm,
        parachain: parachain.parachain,
      },
    ];
    runtime.paras.paras.push(para);
  }

  let tmpfile = `${shell.tempdir()}/${config.relaychain.chain}.json`
  fs.writeFileSync(tmpfile, JSON.stringify(spec, null, 2));

  exec(
    `docker run --rm -v "${tmpfile}":/${config.relaychain.chain}.json ${config.relaychain.image} build-spec --raw --chain=/${config.relaychain.chain}.json --disable-default-bootnode > ${relaychainGenesisFilePath}`
  );

  shell.rm(tmpfile);

  console.log('Relaychain genesis generated at', relaychainGenesisFilePath);
}

const getAddress = (val) => {
  try {
    const addr = decodeAddress(val);
    return encodeAddress(addr);
  } catch { }

  const keyring = new Keyring();
  const pair = keyring.createFromUri(`//${_.startCase(val)}`, null, 'sr25519');

  return pair.address
}

const generateNodeKey = (image) => {
  const res = exec(`docker run --rm ${image} key generate-node-key`)
  return {
    key: res.stdout.trim(),
    address: res.stderr.trim()
  }
}

const generateParachainGenesisFile = (id, image, chain, output, yes) => {
  if (typeof chain === 'string') {
    chain = { base: chain }
  }

  if (!image) {
    return fatal('Missing paras[].image');
  }
  if (!chain) {
    return fatal('Missing paras[].chain');
  }
  if (!chain.base) {
    return fatal('Missing paras[].chain.base');
  }

  const specname = `${chain.base}-${id}.json`;
  const filepath = path.join(output, specname)

  checkOverrideFile(filepath, yes);

  const spec = getChainspec(image, chain.base);

  spec.bootNodes = [];

  const runtime = spec.genesis.runtime;

  runtime.parachainInfo.parachainId = id;

  const endowed = []

  if (chain.sudo && runtime.sudo) {
    runtime.sudo.key = getAddress(chain.sudo)
    endowed.push(runtime.sudo.key)
  }

  if (chain.collators) {
    runtime.collatorSelection.invulnerables = chain.collators.map(getAddress)
    runtime.session.keys = chain.collators.map(x => {
      const addr = getAddress(x);
      return [
        addr, addr, { aura: addr }
      ]
    })

    endowed.push(...runtime.collatorSelection.invulnerables)
  }

  if (endowed.length) {
    const decimals = _.get(spec, 'properties.tokenDecimals[0]') || _.get(spec, 'properties.tokenDecimals') || 15
    const balances = runtime.balances.balances
    const balObj = {}
    for (const [addr, val] of balances) {
      balObj[addr] = val
    }
    for (const addr of endowed) {
      balObj[addr] = (balObj[addr] || 0) + Math.pow(10, decimals)
    }
    runtime.balances.balances = Object.entries(balObj).map(x => x)
  }

  fs.writeFileSync(filepath, JSON.stringify(spec, null, 2));
}

const generateDockerfiles = (config, output, yes) => {
  const relaychainDockerfilePath = path.join(output, 'relaychain.Dockerfile');
  checkOverrideFile(relaychainDockerfilePath, yes);

  const relaychainDockerfile = [
    `FROM ${config.relaychain.image}`,
    'COPY . /app'
  ];

  fs.writeFileSync(relaychainDockerfilePath, relaychainDockerfile.join('\n'));

  for (const para of config.paras) {
    const parachainDockerfilePath = path.join(output, `parachain-${para.id}.Dockerfile`);
    checkOverrideFile(parachainDockerfilePath, yes);

    const parachainDockerfile = [
      `FROM ${para.image}`,
      'COPY . /app'
    ];

    fs.writeFileSync(parachainDockerfilePath, parachainDockerfile.join('\n'));
  }
}

const generate = async (config, { output, yes }) => {
  await cryptoWaitReady();

  if (!config.relaychain.chain) {
    return fatal('Missing relaychain.chain');
  }

  const relaychainGenesisFilePath = path.join(output, `${config.relaychain.chain}.json`);
  checkOverrideFile(relaychainGenesisFilePath, yes);

  const dockerComposePath = path.join(output, 'docker-compose.yml');
  checkOverrideFile(dockerComposePath, yes);

  fs.mkdirSync(output, { recursive: true });

  for (const para of config.paras) {
    generateParachainGenesisFile(para.id, para.image, para.chain, output, yes);
  }

  generateRelaychainGenesisFile(config, relaychainGenesisFilePath, output);

  generateDockerfiles(config, output, yes);

  const dockerCompose = {
    version: '3.7',
    services: {},
    volumes: {},
  };

  const ulimits = {
    nofile: {
      soft: 65536,
      hard: 65536
    }
  }

  let idx = 0;
  for (const node of config.relaychain.nodes) {
    const name = `relaychain-${_.kebabCase(node.name)}`;
    const nodeConfig = {
      ports: [
        `${node.wsPort || 9944 + idx}:9944`,
        `${node.rpcPort || 9933 + idx}:9933`,
        `${node.port || 30333 + idx}:30333`,
      ],
      volumes: [`${name}:/data`],
      build: {
        context: '.',
        dockerfile: 'relaychain.Dockerfile'
      },
      command: [
        '--base-path=/data',
        `--chain=/app/${config.relaychain.chain}.json`,
        '--validator',
        '--ws-external',
        '--rpc-external',
        '--rpc-cors=all',
        `--name=${node.name}`,
        `--${node.name.toLowerCase()}`,
        ...(config.relaychain.flags || []),
        ...(node.flags || []),
      ],
      environment: _.assign({}, config.relaychain.env, node.env),
      ulimits,
    };
    dockerCompose.services[name] = nodeConfig;
    dockerCompose.volumes[name] = null;

    ++idx;
  }

  for (const para of config.paras) {
    let nodeIdx = 0;

    const { key: nodeKey, address: nodeAddress } = generateNodeKey(para.image);

    for (const paraNode of para.nodes) {
      const name = `parachain-${para.id}-${nodeIdx}`;

      const nodeConfig = {
        ports: [
          `${paraNode.wsPort || 9944 + idx}:9944`,
          `${paraNode.rpcPort || 9933 + idx}:9933`,
          `${paraNode.port || 30333 + idx}:30333`,
        ],
        volumes: [`${name}:/acala/data`],
        build: {
          context: '.',
          dockerfile: `parachain-${para.id}.Dockerfile`
        },
        command: [
          '--base-path=/acala/data',
          `--chain=/app/${para.chain.base || para.chain}-${para.id}.json`,
          '--ws-external',
          '--rpc-external',
          '--rpc-cors=all',
          `--name=${name}`,
          '--collator',
          `--parachain-id=${para.id}`,
          ...(para.flags || []),
          ...(paraNode.flags || []),
          nodeIdx === 0 ? `--node-key=${nodeKey}` : `--bootnodes=/dns/parachain-${para.id}-0/tcp/30333/p2p/${nodeAddress}`,
          '--listen-addr=/ip4/0.0.0.0/tcp/30333',
          '--',
          `--chain=/app/${config.relaychain.chain}.json`,
          ...(para.relaychainFlags || []),
          ...(paraNode.relaychainFlags || []),
        ],
        environment: _.assign({}, para.env, paraNode.env),
        ulimits,
      };

      dockerCompose.services[name] = nodeConfig;
      dockerCompose.volumes[name] = null;

      ++nodeIdx;
      ++idx;
    }
  }

  fs.writeFileSync(dockerComposePath, YAML.stringify(dockerCompose));

  console.log('docker-compose.yml generated at', dockerComposePath);
};

yargs(hideBin(process.argv))
  .command(
    'generate [config]',
    'generate the network genesis and docker-compose.yml',
    (yargs) =>
      yargs.positional('config', {
        describe: 'Path to config.yml file',
        default: 'config.yml',
      }),
    (argv) => {
      const { config: configPath } = argv;

      let config;
      try {
        const configFile = fs.readFileSync(configPath, 'utf8');
        config = YAML.parse(configFile);
      } catch (e) {
        console.error('Invalid config file:', configPath);
      }

      generate(config, argv).catch(fatal);
    }
  )
  .option('output', {
    alias: 'o',
    type: 'string',
    default: 'output',
    description: 'The output directory path',
  })
  .option('yes', {
    alias: 'y',
    type: 'boolean',
    description: 'Yes for options',
  })
  .help('h')
  .alias('h', 'help').argv;
