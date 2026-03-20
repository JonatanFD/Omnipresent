import React from "react";

interface OmnipresentProps extends React.SVGProps<SVGSVGElement> {}

export default function Omnipresent(props: OmnipresentProps) {
  return (
    <svg
      id="logo-svg"
      width="512"
      height="512"
      viewBox="0 0 512 512"
      {...props}
    >
      <defs>
        <linearGradient id="bg-gradient" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stopColor="#ffffff" />
          <stop offset="100%" stopColor="#ffffff" />
        </linearGradient>

        <linearGradient id="halo-gradient" x1="0%" y1="0%" x2="100%" y2="0%">
          <stop offset="0%" stopColor="#FFB300" />
          <stop offset="50%" stopColor="#FFD700" />
          <stop offset="100%" stopColor="#FFB300" />
        </linearGradient>

        <filter id="glow" x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="8" result="blur" />
          <feMerge>
            <feMergeNode in="blur" />
            <feMergeNode in="blur" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>

        <filter id="drop-shadow" x="-50%" y="-50%" width="200%" height="200%">
          <feDropShadow
            dx="0"
            dy="15"
            stdDeviation="10"
            floodColor="#000000"
            floodOpacity="0.3"
          />
        </filter>
      </defs>

      <rect
        id="app-bg"
        width="512"
        height="512"
        rx="115"
        fill="url(#bg-gradient)"
      />

      <ellipse
        cx="256"
        cy="115"
        rx="120"
        ry="35"
        fill="none"
        stroke="url(#halo-gradient)"
        strokeWidth="12"
        filter="url(#glow)"
      />
      <ellipse
        cx="256"
        cy="115"
        rx="120"
        ry="35"
        fill="none"
        stroke="#FFFFFF"
        strokeWidth="3"
        opacity="0.8"
      />

      <path
        d="M 179 165 L 179 389 L 235 333 L 277 431 L 319 417 L 277 319 L 333 319 Z"
        fill="#FFFFFF"
        stroke="#1e293b"
        strokeWidth="12"
        strokeLinejoin="round"
        filter="url(#drop-shadow)"
      />
    </svg>
  );
}
