import resolve from '@rollup/plugin-node-resolve';
import copy from 'rollup-plugin-copy';
import { importMetaAssets } from '@web/rollup-plugin-import-meta-assets';
import typescript from '@rollup/plugin-typescript';

export default [
  {
    input: './index.ts',
    output: {
      file: '../dist/index.js',
      format: 'esm',
    },
    plugins: [
      typescript(),
      resolve(),
      importMetaAssets(),
      copy({
        targets: [
          {
            src: './index.html',
            dest: '../dist',
          },
        ],
      }),
    ],
  },
];
