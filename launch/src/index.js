const YAML = require('yaml');
const fs = require('fs');
const path = require('path');
const readline = require('readline-sync');
const shell = require('shelljs');
const { Keyring } = require('@polkadot/api');
const { cryptoWaitReady, encodeAddress } = require('@polkadot/util-crypto');
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

const generateRelaychainGenesis = (config) => {
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
  const res = exec(
    `docker run --rm ${relaychain.image} build-spec --chain=${relaychain.chain} --disable-default-bootnode`
  );

  let spec;

  try {
    spec = JSON.parse(res.stdout);
  } catch (e) {
    return fatal('build spec for relaychain failed', e);
  }

  return spec;
};

const exportParachainGenesis = (paraConfig) => {
  if (!paraConfig.image) {
    return fatal('Missing parachains[].image');
  }

  const args = [];
  if (paraConfig.chain) {
    args.push(`--chain=${paraConfig.chain}`);
  }

  const res2 = exec(`docker run --rm ${paraConfig.image} export-genesis-wasm ${args.join(' ')}`);
  const wasm = res2.stdout.trim();

  if (paraConfig.id) {
    args.push(`--parachain-id=${paraConfig.id}`);
  }
  const res = exec(`docker run --rm ${paraConfig.image} export-genesis-state ${args.join(' ')}`);
  const state = res.stdout.trim();

  return { state, wasm };
};

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

  const spec = generateRelaychainGenesis(config, relaychainGenesisFilePath);

  // clear authorities
  const sessionKeys = spec.genesis.runtime.runtime_genesis_config.palletSession.keys;
  sessionKeys.length = 0;

  // add authorities from config
  const keyring = new Keyring();
  for (const node of config.relaychain.nodes) {
    const { name } = node;
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
    _.merge(spec.genesis.runtime.runtime_genesis_config, config.relaychain.runtime_genesis_config);
  }

  // genesis parachains
  for (const parachain of config.paras) {
    const { wasm, state } = exportParachainGenesis(parachain);
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
    spec.genesis.runtime.runtime_genesis_config.parachainsParas.paras.push(para);
  }


  let tmpfile = `${shell.tempdir()}/${config.relaychain.chain}.json`
  fs.writeFileSync(tmpfile, JSON.stringify(spec, null, 2));

  exec(
    `docker run --rm -v "${tmpfile}":/${config.relaychain.chain}.json ${config.relaychain.image} build-spec --raw --chain=/${config.relaychain.chain}.json --disable-default-bootnode > ${relaychainGenesisFilePath}`
  );

  shell.rm(tmpfile);

  console.log('Relaychain genesis generated at', relaychainGenesisFilePath);

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
      volumes: [`${name}:/data`, '.:/app'],
      image: config.relaychain.image,
      command: [
        '--base-path=/data',
        `--chain=/app/${config.relaychain.chain}.json`,
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
    for (const paraNode of para.nodes) {
      const name = `parachain-${para.id || para.chain}-${nodeIdx}`;
      const nodeConfig = {
        ports: [
          `${paraNode.wsPort || 9944 + idx}:9944`,
          `${paraNode.rpcPort || 9933 + idx}:9933`,
          `${paraNode.port || 30333 + idx}:30333`,
        ],
        volumes: [`${name}:/acala/data`, '.:/app'],
        image: para.image,
        command: [
          '--base-path=/acala/data',
          `--chain=${para.chain}`,
          '--ws-external',
          '--rpc-external',
          '--rpc-cors=all',
          `--name=${name}`,
          '--collator',
          `--parachain-id=${para.id}`,
          ...(para.flags || []),
          ...(paraNode.flags || []),
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
