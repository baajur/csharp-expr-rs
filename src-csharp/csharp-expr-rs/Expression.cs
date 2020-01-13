﻿using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;

namespace csharp_expr_rs
{
    /// <summary>
    /// If non sealed, implement the proper disposable pattern !
    /// </summary>
    public sealed class Expression : IDisposable
    {
        private readonly FFIExpressionHandle _expressionHandle;
        private readonly HashSet<string> _identifiers;

        public Expression(string expression)
            : this(Native.ffi_parse_and_prepare_expr(expression))
        { }

        internal Expression(FFIExpressionHandle expressionFFIPointer)
        {
            _expressionHandle = expressionFFIPointer;
            _identifiers = new HashSet<string>(
                Native.ffi_get_identifiers(_expressionHandle)
                    .AsStringAndDispose()
                    .Split(new[] { '|' }, StringSplitOptions.RemoveEmptyEntries)
                    );
        }

        public string Execute(Dictionary<string, string> identifierValues)
        {
            unsafe
            {
                //var idValues = identifierValues
                //    .Where(kv => _identifiers.Contains(kv.Key))
                //    .Select(kv => new FFIIdentifierKeyValue { key = kv.Key, value = kv.Value })
                //    .ToArray();

                var str = "test";

                fixed (char* ptr = str)
                {
                    var len = (UIntPtr)(str.Length * sizeof(Char));
                    Native.ffi_test(new FFICSharpString { ptr = ptr, len = len }).AsStringAndDispose();
                }


                //var pCh1 = test.GetPinnableReference();
                //byte* pc = (byte*)&pCh1;

            }

            //string result = Native.ffi_exec_expr(_expressionHandle, idValues, (UIntPtr)idValues.Length)
            //    .AsStringAndDispose();
            //return result;
            return null;
        }

        public void Dispose()
        {
            _expressionHandle.Dispose();
        }
    }
}
