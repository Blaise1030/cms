<script setup lang="ts">
import { useId } from "vue";

// The app mark: Lucide's paw-print (ISC) given the cast-silver finish of the old bear.
// The dark under-stroke keeps the highlights readable on a light background.
const uid = useId();
const shapes = `paw-shapes-${uid}`;
const metal = `paw-metal-${uid}`;
</script>

<template>
  <svg viewBox="1 1 22 22" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true" class="paw-mark">
    <defs>
      <g :id="shapes">
        <circle cx="11" cy="4" r="2" />
        <circle cx="18" cy="8" r="2" />
        <circle cx="20" cy="16" r="2" />
        <path d="M9 10a5 5 0 0 1 5 5v3.5a3.5 3.5 0 0 1-6.84 1.045Q6.52 17.48 4.46 16.84A3.5 3.5 0 0 1 5.5 10Z" />
      </g>
      <linearGradient :id="metal" gradientUnits="userSpaceOnUse" x1="2" y1="1" x2="22" y2="23">
        <stop offset="0" stop-color="#c9cad4" />
        <stop offset="0.18" stop-color="#f6f7fb" />
        <stop offset="0.45" stop-color="#c2c3ce" />
        <stop offset="0.72" stop-color="#9a9ba7" />
        <stop offset="1" stop-color="#6f707c" />
      </linearGradient>
    </defs>
    <use :href="`#${shapes}`" stroke="#23242b" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round" opacity="0.7" />
    <use :href="`#${shapes}`" :stroke="`url(#${metal})`" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
  </svg>
</template>
