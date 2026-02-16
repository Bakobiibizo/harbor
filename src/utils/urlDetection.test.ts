import { describe, it, expect } from 'vitest';
import { extractUrls, extractFirstUrl, getDomainFromUrl } from './urlDetection';

describe('urlDetection', () => {
  describe('extractUrls', () => {
    it('should extract a single URL from text', () => {
      const text = 'Check out https://example.com for more info';
      expect(extractUrls(text)).toEqual(['https://example.com']);
    });

    it('should extract multiple URLs from text', () => {
      const text = 'Visit https://example.com and http://test.org/page';
      expect(extractUrls(text)).toEqual(['https://example.com', 'http://test.org/page']);
    });

    it('should return empty array when no URLs are found', () => {
      expect(extractUrls('no urls here')).toEqual([]);
      expect(extractUrls('')).toEqual([]);
    });

    it('should strip trailing punctuation from URLs', () => {
      expect(extractUrls('Go to https://example.com.')).toEqual(['https://example.com']);
      expect(extractUrls('Is it https://example.com?')).toEqual(['https://example.com']);
      expect(extractUrls('Yes, https://example.com!')).toEqual(['https://example.com']);
      expect(extractUrls('Like https://example.com;')).toEqual(['https://example.com']);
      expect(extractUrls('See https://example.com,')).toEqual(['https://example.com']);
    });

    it('should preserve valid URL path segments', () => {
      const url = 'https://example.com/path/to/page';
      expect(extractUrls(`Visit ${url} now`)).toEqual([url]);
    });

    it('should preserve query parameters', () => {
      const url = 'https://example.com/search?q=test&page=1';
      expect(extractUrls(`Result: ${url}`)).toEqual([url]);
    });

    it('should preserve fragment identifiers', () => {
      const url = 'https://example.com/page#section';
      expect(extractUrls(`See ${url}`)).toEqual([url]);
    });

    it('should handle URLs with ports', () => {
      const url = 'http://localhost:3000/api';
      expect(extractUrls(`Server at ${url}`)).toEqual([url]);
    });
  });

  describe('extractFirstUrl', () => {
    it('should return the first URL found', () => {
      const text = 'Visit https://first.com and https://second.com';
      expect(extractFirstUrl(text)).toBe('https://first.com');
    });

    it('should return null when no URL is found', () => {
      expect(extractFirstUrl('no urls here')).toBeNull();
      expect(extractFirstUrl('')).toBeNull();
    });
  });

  describe('getDomainFromUrl', () => {
    it('should extract domain from a standard URL', () => {
      expect(getDomainFromUrl('https://example.com/page')).toBe('example.com');
    });

    it('should strip www. prefix', () => {
      expect(getDomainFromUrl('https://www.example.com')).toBe('example.com');
    });

    it('should preserve subdomains other than www', () => {
      expect(getDomainFromUrl('https://blog.example.com')).toBe('blog.example.com');
    });

    it('should handle URLs with ports', () => {
      expect(getDomainFromUrl('http://localhost:3000')).toBe('localhost');
    });

    it('should return the original string for invalid URLs', () => {
      expect(getDomainFromUrl('not-a-url')).toBe('not-a-url');
    });
  });
});
