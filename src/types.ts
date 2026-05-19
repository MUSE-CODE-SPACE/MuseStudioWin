export type Tab = {
  id: string;
  path: string;
  name: string;
  language: string;
  content: string;
  dirty: boolean;
};

export type DirEntry = {
  name: string;
  path: string;
  is_dir: boolean;
};
