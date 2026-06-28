import json

class IncrementalJsonParser:
    def __init__(self) -> None:
        self._buffer = ""
        self._emitted_indices: set[int] = set()
        
    def add_chunk(self, text: str) -> list[dict[str, object]]:
        self._buffer += text
        return self._extract_objects()
        
    def _extract_objects(self) -> list[dict[str, object]]:
        extracted = []
        
        # Fast path to find potential JSON objects representing translation items
        # We look for "index": <number> and "translation": "..."
        
        start_idx = 0
        while True:
            obj_start = self._buffer.find("{", start_idx)
            if obj_start == -1:
                break
                
            # Naive bracket matching for object bounds
            brace_count = 0
            in_string = False
            escape_next = False
            obj_end = -1
            
            for i in range(obj_start, len(self._buffer)):
                char = self._buffer[i]
                
                if escape_next:
                    escape_next = False
                    continue
                    
                if char == "\\":
                    escape_next = True
                    continue
                    
                if char == '"':
                    in_string = not in_string
                    continue
                    
                if not in_string:
                    if char == "{":
                        brace_count += 1
                    elif char == "}":
                        brace_count -= 1
                        if brace_count == 0:
                            obj_end = i
                            break
                            
            if obj_end != -1:
                # We found a complete object string
                candidate_str = self._buffer[obj_start:obj_end + 1]
                try:
                    obj = json.loads(candidate_str)
                    if isinstance(obj, dict) and "index" in obj and "translation" in obj:
                        index = obj.get("index")
                        if isinstance(index, (int, str)) and str(index).isdigit():
                            idx_val = int(index)
                            if idx_val not in self._emitted_indices:
                                self._emitted_indices.add(idx_val)
                                extracted.append(obj)
                except json.JSONDecodeError:
                    pass
                    
                start_idx = obj_start + 1 # Advance slightly to find nested objects if necessary, though typical is array of objects
            else:
                # No complete object found from this start brace, try the next one just in case
                # Actually, if we didn't finish an object, we need more data.
                # But wait, what if the first brace is not the start of the object we want, but the root `{` of the whole response?
                # We MUST advance start_idx to search inside it!
                start_idx = obj_start + 1
        
        return extracted
