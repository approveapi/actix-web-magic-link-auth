#!/bin/bash

set -e

function require() {
    if [ -z "$1" ]
    then
        echo "Missing argument: $2."
        exit 1
    fi
}

usage="$(basename "$0"): A CLI to work with ApproveAPI.

\n\nwhere:\n
    \t--help,  -h       show this help text\n
    \t--key  , -k       API key found here: https://dashboard.approveapi.com/api_keys\n
    \t--user,  -u       user(s) to send an approval prompt\n
    \t--title, -t       title for prompt\n
    \t--body,  -b       body for prompt\n
    
    
\nexample:\n
    \t./approve.sh --key=1234 --user=bobby@acme.co --body='Allow access to X?'\n
"

for i in "$@"
do
    case $i in
        -h|--help)
        echo -e $usage
        exit 0
        ;;
        -k=*|--key=*)
        KEY="${i#*=}"
        shift
        ;;
        -u=*|--user=*)
        AUSER="${i#*=}"
        shift
        ;;
        -t=*|--title=*)
        TITLE="${i#*=}"
        shift
        ;;
        -b=*|--body=*)
        BODY="${i#*=}"
        shift
        ;;
        *)
        echo -e $usage # unknown option
        ;;
    esac
done

require "$KEY" "You need an API key. Find one at https://dashboard.approveapi.com/api_keys"
require "$AUSER" "You need to specify a user for the prompt"
require "$TITLE" "You need to specify a title for the prompt"
require "$BODY" "You need to specify a body for the prompt"

function parse_response() {
    local response="$1"
    local hasAnswer=$(python -c "import sys, json; j = '''$response'''; response = json.loads(j); print '' if 'answer' not in response or response['answer']['result'] is None else 'true'")

    if [ -z "$hasAnswer" ]
    then
        echo -e "ApproveAPI did not return successfully.\n\n$response"
        exit 1
    fi

    local result=$(python -c "import sys, json; j = '''$response'''; response = json.loads(j); print 'approved' if response['answer']['result'] == True else ''")

    if [ -z "$result" ]
    then
        echo -e "Rejected"
        exit 1
    fi

    >&2 echo -e "Approved"
    exit 0
}

function send_prompt() {
    local api_response=$(curl -s "https://approve.sh/prompt" -u "$KEY:" -d long_poll=true -d user="$AUSER" -d title="$TITLE" -d body="$BODY" -d approve_text="Authorize" -d reject_text="Reject")

    parse_response "$api_response"
} 

send_prompt